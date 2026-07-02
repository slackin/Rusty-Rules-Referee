#!/usr/bin/env python3
"""LocalAI benchmarking + tuning harness.

Measures streaming latency (TTFT), decode throughput (tok/s), prompt/completion
token counts, and captures full outputs so tweaks can be compared objectively.

Only uses the Python standard library (urllib) so no pip install is required.

Examples
--------
  # Baseline run across the coding suite, labelled "baseline"
  python localai_bench.py run --label baseline

  # After you change the YAML in the web UI and reload the model:
  python localai_bench.py run --label flash-attn-on

  # Single quick prompt
  python localai_bench.py run --label quick --suite quick

  # Long-context "needle in a haystack" recall test at ~8000 tokens of filler
  python localai_bench.py context --label ctx8k --ctx-tokens 8000

  # Print a comparison table of every run recorded so far
  python localai_bench.py summary

  # Show the full text output of a specific run id
  python localai_bench.py show <run_id>

Results are appended to bench/results.jsonl next to this script.
"""
from __future__ import annotations

import argparse
import json
import os
import sys
import time
import urllib.request
import urllib.error
import uuid
from datetime import datetime, timezone

DEFAULT_HOST = "http://10.42.1.40:8080"
DEFAULT_MODEL = "qwen3-coder-reap-25b-a3b-i1"
RESULTS_PATH = os.path.join(os.path.dirname(os.path.abspath(__file__)), "results.jsonl")

# --- Prompt suites -----------------------------------------------------------
SUITES: dict[str, list[dict]] = {
    "quick": [
        {"name": "reverse_list", "prompt": "Reverse a singly linked list in Rust. Return only the code."},
    ],
    "coding": [
        {
            "name": "fib_iter",
            "prompt": "Write a Rust function that returns the nth Fibonacci number iteratively, with overflow handling. Return only the code.",
        },
        {
            "name": "debug_borrow",
            "prompt": (
                "This Rust code fails to compile with a borrow error:\n\n"
                "```rust\nfn main() {\n    let mut v = vec![1, 2, 3];\n    for x in &v {\n        v.push(*x);\n    }\n}\n```\n\n"
                "Explain the error briefly and give a corrected version."
            ),
        },
        {
            "name": "sql_query",
            "prompt": (
                "Given a SQLite table `penalties(id, type, client_id, admin_id, duration, reason, time_add, time_expire, inactive)`, "
                "write a query that returns the 10 most recent active bans (type='Ban', inactive=0) with the admin's action count. Return only SQL."
            ),
        },
        {
            "name": "refactor_async",
            "prompt": (
                "Refactor this blocking Rust code to async using tokio, running the two fetches concurrently:\n\n"
                "```rust\nfn load() -> (String, String) {\n    let a = fetch(\"a\");\n    let b = fetch(\"b\");\n    (a, b)\n}\n```\nReturn only the code."
            ),
        },
        {
            "name": "explain_regex",
            "prompt": "Explain what this regex matches, concisely: ^(?:[a-z0-9!#$%&'*+/=?^_`{|}~-]+(?:\\.[a-z0-9!#$%&'*+/=?^_`{|}~-]+)*)@(?:[a-z0-9](?:[a-z0-9-]*[a-z0-9])?\\.)+[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$",
        },
    ],
}


def now_iso() -> str:
    return datetime.now(timezone.utc).isoformat()


def stream_chat(host: str, model: str, prompt: str, *, temperature: float,
                top_p: float, top_k: int, max_tokens: int,
                system: str | None, timeout: float) -> dict:
    """Send a streaming chat completion and collect timing + text.

    Returns a dict with ttft, total_time, decode_tps, chunk_count, text, error.
    """
    body: dict = {
        "model": model,
        "messages": [],
        "temperature": temperature,
        "top_p": top_p,
        "top_k": top_k,
        "max_tokens": max_tokens,
        "stream": True,
    }
    if system:
        body["messages"].append({"role": "system", "content": system})
    body["messages"].append({"role": "user", "content": prompt})

    data = json.dumps(body).encode("utf-8")
    req = urllib.request.Request(
        f"{host}/v1/chat/completions",
        data=data,
        headers={
            "Content-Type": "application/json",
            "Accept": "text/event-stream",
            # Ask LocalAI to include server-side prefill/generation timings (ms).
            "Extra-Usage": "true",
        },
        method="POST",
    )

    start = time.perf_counter()
    ttft: float | None = None
    chunk_count = 0
    pieces: list[str] = []
    reasoning_pieces: list[str] = []
    reasoning_chunks = 0
    usage: dict | None = None
    error: str | None = None

    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            for raw in resp:
                line = raw.decode("utf-8", "replace").strip()
                if not line or not line.startswith("data:"):
                    continue
                payload = line[len("data:"):].strip()
                if payload == "[DONE]":
                    break
                try:
                    obj = json.loads(payload)
                except json.JSONDecodeError:
                    continue
                if isinstance(obj.get("usage"), dict):
                    usage = obj["usage"]
                choices = obj.get("choices") or []
                if not choices:
                    continue
                delta = choices[0].get("delta") or {}
                # Reasoning models stream chain-of-thought separately from content.
                rpiece = delta.get("reasoning") or delta.get("reasoning_content")
                if rpiece:
                    if ttft is None:
                        ttft = time.perf_counter() - start
                    reasoning_chunks += 1
                    reasoning_pieces.append(rpiece)
                piece = delta.get("content")
                if piece:
                    if ttft is None:
                        ttft = time.perf_counter() - start
                    chunk_count += 1
                    pieces.append(piece)
    except urllib.error.HTTPError as e:
        error = f"HTTP {e.code}: {e.read().decode('utf-8', 'replace')[:400]}"
    except Exception as e:  # noqa: BLE001 - report any transport error
        error = f"{type(e).__name__}: {e}"

    total = time.perf_counter() - start
    text = "".join(pieces)
    reasoning_text = "".join(reasoning_pieces)
    decode_time = (total - ttft) if ttft is not None else None
    # Prefer server-reported completion tokens; fall back to streamed chunk count
    # (reasoning + content) so reasoning models report true generation speed.
    streamed = chunk_count + reasoning_chunks
    completion_tokens = (usage or {}).get("completion_tokens") or streamed
    decode_tps = (completion_tokens / decode_time) if decode_time and decode_time > 0 else None
    # Server-side timings (ms) from the Extra-Usage header, when available.
    prefill_ms = (usage or {}).get("timing_prompt_processing")
    gen_ms = (usage or {}).get("timing_token_generation")
    server_gen_tps = None
    if gen_ms and completion_tokens:
        server_gen_tps = round(completion_tokens / (gen_ms / 1000.0), 2)

    return {
        "ttft": round(ttft, 3) if ttft is not None else None,
        "total_time": round(total, 3),
        "decode_tps": round(decode_tps, 2) if decode_tps else None,
        "server_gen_tps": server_gen_tps,
        "prefill_ms": round(prefill_ms, 1) if prefill_ms else None,
        "gen_ms": round(gen_ms, 1) if gen_ms else None,
        "chunk_count": chunk_count,
        "reasoning_chunks": reasoning_chunks,
        "completion_tokens": completion_tokens,
        "prompt_tokens": (usage or {}).get("prompt_tokens"),
        "text": text,
        "reasoning_text": reasoning_text,
        "error": error,
    }


def append_result(rec: dict) -> None:
    with open(RESULTS_PATH, "a", encoding="utf-8") as f:
        f.write(json.dumps(rec) + "\n")


def load_results() -> list[dict]:
    if not os.path.exists(RESULTS_PATH):
        return []
    out = []
    with open(RESULTS_PATH, encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if line:
                out.append(json.loads(line))
    return out


def cmd_run(args) -> None:
    suite = SUITES.get(args.suite)
    if not suite:
        print(f"Unknown suite '{args.suite}'. Options: {', '.join(SUITES)}")
        sys.exit(1)

    print(f"== {args.label} | model={args.model} | suite={args.suite} | "
          f"temp={args.temperature} top_p={args.top_p} top_k={args.top_k} ==")
    ttfts, tpss = [], []
    for case in suite:
        r = stream_chat(
            args.host, args.model, case["prompt"],
            temperature=args.temperature, top_p=args.top_p, top_k=args.top_k,
            max_tokens=args.max_tokens, system=args.system, timeout=args.timeout,
        )
        run_id = uuid.uuid4().hex[:8]
        rec = {
            "run_id": run_id,
            "ts": now_iso(),
            "label": args.label,
            "model": args.model,
            "suite": args.suite,
            "case": case["name"],
            "params": {
                "temperature": args.temperature,
                "top_p": args.top_p,
                "top_k": args.top_k,
                "max_tokens": args.max_tokens,
                "system": args.system,
            },
            **{k: v for k, v in r.items() if k != "text"},
            "text": r["text"],
        }
        append_result(rec)
        if r["error"]:
            print(f"  [{run_id}] {case['name']:<14} ERROR: {r['error']}")
            continue
        ttfts.append(r["ttft"] or 0)
        if r["decode_tps"]:
            tpss.append(r["decode_tps"])
        prefill = f" prefill={r['prefill_ms']}ms" if r.get("prefill_ms") else ""
        gen = f" gen={r['server_gen_tps']}tok/s" if r.get("server_gen_tps") else ""
        think = f" think={r['reasoning_chunks']}" if r.get("reasoning_chunks") else ""
        print(f"  [{run_id}] {case['name']:<14} ttft={r['ttft']}s "
              f"decode={r['decode_tps']} tok/s tokens={r['completion_tokens']} "
              f"total={r['total_time']}s{think}{prefill}{gen}")
    if ttfts:
        avg_ttft = sum(ttfts) / len(ttfts)
        avg_tps = sum(tpss) / len(tpss) if tpss else 0
        print(f"  -- avg ttft={avg_ttft:.2f}s  avg decode={avg_tps:.2f} tok/s --")


def cmd_context(args) -> None:
    # Needle-in-a-haystack: hide a fact in filler and ask the model to recall it.
    secret = f"PLUM-{uuid.uuid4().hex[:6].upper()}"
    filler_unit = ("The referee service tails the Urban Terror game log and "
                   "dispatches parsed events to plugins over an async channel. ")
    # ~1 token per ~0.75 words; aim for approx target token count of filler.
    approx_words_needed = int(args.ctx_tokens * 0.75)
    unit_words = len(filler_unit.split())
    reps = max(1, approx_words_needed // unit_words)
    haystack_pre = filler_unit * (reps // 2)
    haystack_post = filler_unit * (reps - reps // 2)
    prompt = (
        haystack_pre
        + f"\n\nIMPORTANT: The authorization codeword is {secret}. Remember it.\n\n"
        + haystack_post
        + "\n\nQuestion: What is the authorization codeword mentioned above? "
          "Answer with only the codeword."
    )
    print(f"== {args.label} | context recall | target~{args.ctx_tokens} tok | secret={secret} ==")
    r = stream_chat(
        args.host, args.model, prompt,
        temperature=0.0, top_p=1.0, top_k=-1,
        max_tokens=32, system=args.system, timeout=args.timeout,
    )
    run_id = uuid.uuid4().hex[:8]
    found = secret in (r["text"] or "")
    rec = {
        "run_id": run_id,
        "ts": now_iso(),
        "label": args.label,
        "model": args.model,
        "suite": "context",
        "case": f"recall_{args.ctx_tokens}",
        "params": {"ctx_tokens": args.ctx_tokens, "secret": secret},
        **{k: v for k, v in r.items() if k != "text"},
        "recall_ok": found,
        "text": r["text"],
    }
    append_result(rec)
    if r["error"]:
        print(f"  [{run_id}] ERROR: {r['error']}")
        return
    print(f"  [{run_id}] prompt_tokens={r['prompt_tokens']} ttft={r['ttft']}s "
          f"recall={'PASS' if found else 'FAIL'} -> {r['text']!r}")


def cmd_summary(args) -> None:
    rows = load_results()
    if not rows:
        print("No results yet.")
        return
    # Group by (label, suite, case) most recent first for readability.
    print(f"{'label':<18}{'case':<16}{'ttft':>7}{'tok/s':>8}{'tokens':>8}{'total':>8}  run_id")
    print("-" * 78)
    for r in rows[-args.last:]:
        ttft = r.get("ttft")
        tps = r.get("decode_tps")
        extra = ""
        if r.get("suite") == "context":
            extra = "  recall=" + ("PASS" if r.get("recall_ok") else "FAIL")
        print(f"{r.get('label',''):<18}{r.get('case',''):<16}"
              f"{(ttft if ttft is not None else '-'):>7}"
              f"{(tps if tps is not None else '-'):>8}"
              f"{(r.get('completion_tokens') or '-'):>8}"
              f"{(r.get('total_time') or '-'):>8}  {r.get('run_id','')}{extra}")


def cmd_show(args) -> None:
    for r in load_results():
        if r.get("run_id") == args.run_id:
            print(f"# {r.get('label')} / {r.get('case')} ({r.get('run_id')})")
            print(f"# ttft={r.get('ttft')}s decode={r.get('decode_tps')} tok/s "
                  f"tokens={r.get('completion_tokens')}")
            print("-" * 60)
            print(r.get("text", ""))
            return
    print(f"run_id {args.run_id} not found")


def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(description="LocalAI bench + tuning harness")
    p.add_argument("--host", default=DEFAULT_HOST)
    p.add_argument("--model", default=DEFAULT_MODEL)
    p.add_argument("--timeout", type=float, default=300.0)
    sub = p.add_subparsers(dest="cmd", required=True)

    r = sub.add_parser("run", help="run a prompt suite")
    r.add_argument("--label", required=True)
    r.add_argument("--suite", default="coding", choices=list(SUITES))
    r.add_argument("--temperature", type=float, default=0.7)
    r.add_argument("--top_p", type=float, default=0.8)
    r.add_argument("--top_k", type=int, default=20)
    r.add_argument("--max_tokens", type=int, default=512)
    r.add_argument("--system", default=None)
    r.set_defaults(func=cmd_run)

    c = sub.add_parser("context", help="long-context recall test")
    c.add_argument("--label", required=True)
    c.add_argument("--ctx-tokens", type=int, default=8000, dest="ctx_tokens")
    c.add_argument("--system", default=None)
    c.set_defaults(func=cmd_context)

    s = sub.add_parser("summary", help="print comparison table")
    s.add_argument("--last", type=int, default=50)
    s.set_defaults(func=cmd_summary)

    sh = sub.add_parser("show", help="print full text of a run_id")
    sh.add_argument("run_id")
    sh.set_defaults(func=cmd_show)
    return p


def main() -> None:
    args = build_parser().parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
