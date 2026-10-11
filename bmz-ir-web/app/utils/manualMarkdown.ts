import { Marked, type Tokens } from 'marked'

export type ManualLocale = 'ja' | 'en'

export interface ManualPage {
  slug: string
  title: string
  summary: string
  markdown: string
}

const FILE_PATTERN = /\/manual\/(?<locale>ja|en)\/(?<order>\d+)-(?<slug>[a-z0-9-]+)\.md$/u
/** 原稿内のほかのページへのリンク（`02-setup.md` など）。 */
export const MANUAL_PAGE_LINK_PATTERN = /^(?:\.\/)?\d+-(?<slug>[a-z0-9-]+)\.md(?<hash>#.*)?$/u

function parsePage(locale: ManualLocale, slug: string, source: string): ManualPage {
  const normalized = source.replace(/\r\n/gu, '\n')
  const heading = /^# (?<title>.+)\n+/u.exec(normalized)
  const markdown = heading ? normalized.slice(heading[0].length) : normalized
  const summary = (markdown.split('\n\n', 1)[0] ?? '')
    .replace(/\n/gu, locale === 'ja' ? '' : ' ')
    .replace(/\[(?<text>[^\]]+)\]\([^)]*\)/gu, '$<text>')
    .replace(/\*\*/gu, '')
  return { slug, title: heading?.groups?.title?.trim() ?? slug, summary, markdown }
}

/** `manual/<locale>/NN-slug.md` のパスと本文から、言語ごとに並べたページ一覧を作る。 */
export function buildManualIndex(
  sources: Record<string, string>,
): Record<ManualLocale, ManualPage[]> {
  const entries: Record<ManualLocale, { order: number; page: ManualPage }[]> = { ja: [], en: [] }
  for (const [path, source] of Object.entries(sources)) {
    const match = FILE_PATTERN.exec(path.replace(/\\/gu, '/'))
    if (!match?.groups) continue
    const locale = match.groups.locale as ManualLocale
    entries[locale].push({
      order: Number(match.groups.order),
      page: parsePage(locale, match.groups.slug!, source),
    })
  }
  return {
    ja: entries.ja.sort((a, b) => a.order - b.order).map((entry) => entry.page),
    en: entries.en.sort((a, b) => a.order - b.order).map((entry) => entry.page),
  }
}

/** 原稿がない言語では英語版を使う。 */
export function manualLocaleFor(locale: string): ManualLocale {
  return locale === 'ja' ? 'ja' : 'en'
}

function escapeHtml(value: string): string {
  return value
    .replace(/&/gu, '&amp;')
    .replace(/</gu, '&lt;')
    .replace(/>/gu, '&gt;')
    .replace(/"/gu, '&quot;')
}

// 日本語の文の途中の改行は、表示すると空白になるため取り除く。英語の改行は空白として残す。
const CJK_SOFT_BREAK = /(?<=\P{ASCII})\n|\n(?=\P{ASCII})/gu

/**
 * 原稿の Markdown を HTML にする。ほかのページへの相対リンクは `pagePath` で
 * サイト内の URL に変換し、外部リンクは別タブで開く。
 */
export function renderManualMarkdown(markdown: string, pagePath: (slug: string) => string): string {
  const marked = new Marked({
    gfm: true,
    walkTokens(token) {
      if (token.type === 'text' && !('tokens' in token && token.tokens)) {
        token.text = token.text.replace(CJK_SOFT_BREAK, '')
      }
    },
    renderer: {
      link({ href, title, tokens }: Tokens.Link) {
        const text = this.parser.parseInline(tokens)
        const titleAttribute = title ? ` title="${escapeHtml(title)}"` : ''
        const page = MANUAL_PAGE_LINK_PATTERN.exec(href)
        if (page?.groups) {
          const target = pagePath(page.groups.slug!) + (page.groups.hash ?? '')
          return `<a href="${escapeHtml(target)}" data-manual-link${titleAttribute}>${text}</a>`
        }
        return `<a href="${escapeHtml(href)}" target="_blank" rel="noopener noreferrer"${titleAttribute}>${text}</a>`
      },
    },
  })
  return marked.parse(markdown, { async: false })
}
