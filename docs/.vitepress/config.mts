import { defineConfig } from 'vitepress'

export default defineConfig({
  title: 'Rusty Rules Referee',
  description: 'Documentation for R3 — a high-performance game server administration bot for Urban Terror 4.3',
  head: [
    ['link', { rel: 'icon', href: '/favicon.svg' }]
  ],

  themeConfig: {
    logo: '/logo.svg',
    siteTitle: 'R3 Docs',

    nav: [
      { text: 'Guide', link: '/guide/introduction' },
      { text: 'Plugins', link: '/plugins/' },
      { text: 'Commands', link: '/commands/' },
      { text: 'Dashboard', link: '/dashboard/' },
      { text: 'Changelog', link: '/changelog' }
    ],

    sidebar: {
      '/guide/': [
        {
          text: 'Getting Started',
          items: [
            { text: 'Introduction', link: '/guide/introduction' },
            { text: 'Installation', link: '/guide/installation' },
            { text: 'Quick Start', link: '/guide/quick-start' },
            { text: 'Configuration', link: '/guide/configuration' }
          ]
        }
      ],
      '/plugins/': [
        {
          text: 'Plugins',
          items: [
            { text: 'Overview', link: '/plugins/' }
          ]
        },
        {
          text: 'Core Administration',
          items: [
            { text: 'Admin', link: '/plugins/admin' },
            { text: 'PowerAdminUrt', link: '/plugins/poweradminurt' }
          ]
        },
        {
          text: 'Moderation',
          items: [
            { text: 'Censor', link: '/plugins/censor' },
            { text: 'CensorUrt', link: '/plugins/censorurt' },
            { text: 'SpamControl', link: '/plugins/spamcontrol' },
            { text: 'Team Kill (TK)', link: '/plugins/tk' },
            { text: 'SpawnKill', link: '/plugins/spawnkill' }
          ]
        },
        {
          text: 'Player Management',
          items: [
            { text: 'Welcome', link: '/plugins/welcome' },
            { text: 'MakeRoom', link: '/plugins/makeroom' },
            { text: 'NickReg', link: '/plugins/nickreg' },
            { text: 'NameChecker', link: '/plugins/namechecker' },
            { text: 'SpecChecker', link: '/plugins/specchecker' },
            { text: 'Login', link: '/plugins/login' },
            { text: 'Follow', link: '/plugins/follow' }
          ]
        },
        {
          text: 'Anti-Abuse',
          items: [
            { text: 'AFK', link: '/plugins/afk' },
            { text: 'PingWatch', link: '/plugins/pingwatch' },
            { text: 'VPN Check', link: '/plugins/vpncheck' },
            { text: 'CountryFilter', link: '/plugins/countryfilter' },
            { text: 'Callvote', link: '/plugins/callvote' }
          ]
        },
        {
          text: 'Statistics',
          items: [
            { text: 'Stats', link: '/plugins/stats' },
            { text: 'XLRstats', link: '/plugins/xlrstats' },
            { text: 'HeadshotCounter', link: '/plugins/headshotcounter' },
            { text: 'Spree', link: '/plugins/spree' },
            { text: 'FirstKill', link: '/plugins/firstkill' },
            { text: 'FlagAnnounce', link: '/plugins/flagannounce' }
          ]
        },
        {
          text: 'Chat & Logging',
          items: [
            { text: 'ChatLogger', link: '/plugins/chatlogger' },
            { text: 'CustomCommands', link: '/plugins/customcommands' }
          ]
        },
        {
          text: 'Server Management',
          items: [
            { text: 'Adv', link: '/plugins/adv' },
            { text: 'Scheduler', link: '/plugins/scheduler' },
            { text: 'MapConfig', link: '/plugins/mapconfig' }
          ]
        }
      ],
      '/commands/': [
        {
          text: 'Command Reference',
          items: [
            { text: 'All Commands', link: '/commands/' }
          ]
        }
      ],
      '/dashboard/': [
        {
          text: 'Web Dashboard',
          items: [
            { text: 'Overview', link: '/dashboard/' },
            { text: 'API Reference', link: '/dashboard/api-reference' }
          ]
        }
      ],
      '/development/': [
        {
          text: 'Development',
          items: [
            { text: 'Contributing', link: '/development/contributing' },
            { text: 'Architecture', link: '/development/architecture' },
            { text: 'Adding Plugins', link: '/development/adding-plugins' }
          ]
        }
      ]
    },

    socialLinks: [
      { icon: 'github', link: 'https://github.com/' }
    ],

    search: {
      provider: 'local'
    },

    footer: {
      message: 'Released under the GPL-2.0 License.',
      copyright: 'Copyright © 2026 Rusty Rules Referee'
    }
  }
})
