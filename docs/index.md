---
layout: home

hero:
  name: Rusty Rules Referee
  text: Game Server Administration Bot
  tagline: A high-performance Urban Terror 4.3 server admin bot written in Rust — fast, safe, and extensible with 30 plugins.
  actions:
    - theme: brand
      text: Get Started
      link: /guide/introduction
    - theme: brand
      text: Download Installer
      link: https://r3.pugbot.net/api/updates/install-r3.sh
    - theme: alt
      text: Command Reference
      link: /commands/
    - theme: alt
      text: View on GitHub
      link: https://github.com/

features:
  - icon: ⚡
    title: Blazing Fast
    details: Rust-native log parsing and async event handling powered by tokio. No GC pauses, no memory leaks in a 24/7 service.
  - icon: 🔌
    title: 30 Plugins
    details: Moderation, statistics, anti-abuse, chat logging, server management, and more — all configurable and extensible.
  - icon: 🎯
    title: Real-Time Monitoring
    details: Async log file tailing with rotation detection. Events are parsed and dispatched to plugins in milliseconds.
  - icon: 🌐
    title: Web Dashboard
    details: SvelteKit-based admin panel with live scoreboard, chat, RCON console, player management, and XLRstats leaderboards.
  - icon: 🗄️
    title: Dual Database Support
    details: SQLite for simplicity or MySQL for scale. Automatic migrations on startup keep your schema current.
  - icon: 📊
    title: XLRstats
    details: Extended player statistics with ELO-based skill tracking, weapon stats, map stats, and leaderboards.
---

<section class="r3-demo-video">
  <div class="r3-demo-video__inner">
    <h2 class="r3-demo-video__title">See R3 in action</h2>
    <p class="r3-demo-video__subtitle">A two-minute tour of the web dashboard — live scoreboard, player management, penalties, RCON console, and the full plugin configuration UI.</p>
    <video
      class="r3-demo-video__player"
      controls
      muted
      autoplay
      playsinline
      preload="metadata"
      poster="https://r3.pugbot.net/media/r3-demo-poster.jpg"
    >
      <source src="https://r3.pugbot.net/media/r3-demo-720p.mp4" type="video/mp4" media="(max-width: 900px)" />
      <source src="https://r3.pugbot.net/media/r3-demo-1080p.mp4" type="video/mp4" media="(max-width: 1800px)" />
      <source src="https://r3.pugbot.net/media/r3-demo-1440p.mp4" type="video/mp4" />
      Your browser doesn't support HTML5 video.
      <a href="https://r3.pugbot.net/media/r3-demo-1080p.mp4">Download the demo (1080p MP4).</a>
    </video>
    <p class="r3-demo-video__links">
      Also available:
      <a href="https://r3.pugbot.net/media/r3-demo-720p.mp4">720p</a>&nbsp;·
      <a href="https://r3.pugbot.net/media/r3-demo-1080p.mp4">1080p</a>&nbsp;·
      <a href="https://r3.pugbot.net/media/r3-demo-1440p.mp4">1440p</a>
    </p>
  </div>
</section>
