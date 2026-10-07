// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

export default defineConfig({
  site: 'https://quota.pinkpixel.dev',
  integrations: [
    starlight({
      title: 'Quota',
      description: 'Track AI usage across Claude Code, Codex, Copilot, Cursor, Antigravity, Kiro, Grok and OpenCode Go.',
      logo: { src: './src/assets/icon.png', alt: 'Quota' },
      favicon: '/favicon.png',
      head: [{ tag: 'link', attrs: { rel: 'apple-touch-icon', href: '/apple-touch-icon.png' } }],
      social: [{ icon: 'github', label: 'GitHub', href: 'https://github.com/pinkpixel-dev/quota' }],
      expressiveCode: {
        themes: ['github-dark-default', 'github-light-default'],
        defaultProps: { frame: 'code' },
        styleOverrides: {
          borderRadius: '8px',
          borderColor: 'var(--q-line)',
          codeBackground: 'var(--q-code-bg)',
          frames: {
            editorBackground: 'var(--q-code-bg)',
            editorTabBarBackground: 'var(--q-panel)',
            editorActiveTabBackground: 'var(--q-code-bg)',
            frameBoxShadowCssValue: 'none',
          },
        },
      },
      editLink: { baseUrl: 'https://github.com/pinkpixel-dev/quota/edit/main/website/' },
      customCss: [
        '@fontsource-variable/geist',
        '@fontsource-variable/geist-mono',
        './src/styles/tokens.css',
        './src/styles/starlight.css',
      ],
      sidebar: [
        {
          label: 'Start here',
          items: [
            { label: 'Getting started', slug: 'docs' },
            { label: 'Providers', slug: 'docs/providers' },
          ],
        },
        {
          label: 'Apps',
          items: [
            { label: 'Desktop app', slug: 'docs/desktop' },
            { label: 'VS Code extension', slug: 'docs/vscode' },
            { label: 'quota-cli', slug: 'docs/cli' },
            { label: 'Herdr plugin', slug: 'docs/herdr' },
          ],
        },
        {
          label: 'More',
          items: [{ label: 'Privacy and storage', slug: 'docs/privacy' }],
        },
      ],
    }),
  ],
});
