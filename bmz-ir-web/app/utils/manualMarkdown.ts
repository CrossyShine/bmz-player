import { Marked, type Tokens } from 'marked'

/** 原稿のある言語。`manual/<言語>/` のディレクトリ名と IR Web の locale code に一致する。 */
export const MANUAL_LOCALES = ['ja', 'en', 'ko', 'zh-CN', 'zh-TW', 'zh-HK'] as const
export type ManualLocale = (typeof MANUAL_LOCALES)[number]

/** 改行を空白として扱わず、そのまま詰める言語。韓国語は単語を空白で区切るため含めない。 */
function joinsLinesWithoutSpace(locale: ManualLocale): boolean {
  return locale === 'ja' || locale.startsWith('zh-')
}

export interface ManualPage {
  slug: string
  title: string
  summary: string
  markdown: string
}

const FILE_PATTERN = /\/manual\/(?<locale>[A-Za-z-]+)\/(?<order>\d+)-(?<slug>[a-z0-9-]+)\.md$/u
/** 原稿内のほかのページへのリンク（`02-setup.md` など）。 */
export const MANUAL_PAGE_LINK_PATTERN = /^(?:\.\/)?\d+-(?<slug>[a-z0-9-]+)\.md(?<hash>#.*)?$/u

function parsePage(locale: ManualLocale, slug: string, source: string): ManualPage {
  const normalized = source.replace(/\r\n/gu, '\n')
  const heading = /^# (?<title>.+)\n+/u.exec(normalized)
  const markdown = heading ? normalized.slice(heading[0].length) : normalized
  const summary = (markdown.split('\n\n', 1)[0] ?? '')
    .replace(/\n/gu, joinsLinesWithoutSpace(locale) ? '' : ' ')
    .replace(/\[(?<text>[^\]]+)\]\([^)]*\)/gu, '$<text>')
    .replace(/\*\*/gu, '')
  return { slug, title: heading?.groups?.title?.trim() ?? slug, summary, markdown }
}

/** `manual/<locale>/NN-slug.md` のパスと本文から、言語ごとに並べたページ一覧を作る。 */
export function buildManualIndex(
  sources: Record<string, string>,
): Record<ManualLocale, ManualPage[]> {
  type Entry = { order: number; page: ManualPage }
  const entries = Object.fromEntries(
    MANUAL_LOCALES.map((locale): [ManualLocale, Entry[]] => [locale, []]),
  ) as Record<ManualLocale, Entry[]>
  for (const [path, source] of Object.entries(sources)) {
    const match = FILE_PATTERN.exec(path.replace(/\\/gu, '/'))
    const locale = match?.groups?.locale
    if (!match?.groups || !isManualLocale(locale)) continue
    entries[locale].push({
      order: Number(match.groups.order),
      page: parsePage(locale, match.groups.slug!, source),
    })
  }
  return Object.fromEntries(
    MANUAL_LOCALES.map((locale) => [
      locale,
      entries[locale].sort((a, b) => a.order - b.order).map((entry) => entry.page),
    ]),
  ) as Record<ManualLocale, ManualPage[]>
}

function isManualLocale(locale: string | undefined): locale is ManualLocale {
  return (MANUAL_LOCALES as readonly string[]).includes(locale ?? '')
}

/** 原稿のない言語、またはページが1つもない言語では英語版を使う。 */
export function manualLocaleFor(
  locale: string,
  pages: Record<ManualLocale, ManualPage[]>,
): ManualLocale {
  return isManualLocale(locale) && pages[locale].length > 0 ? locale : 'en'
}

function escapeHtml(value: string): string {
  return value
    .replace(/&/gu, '&amp;')
    .replace(/</gu, '&lt;')
    .replace(/>/gu, '&gt;')
    .replace(/"/gu, '&quot;')
}

// 日本語・中国語の文の途中の改行は、表示すると空白になるため取り除く。英語・韓国語の改行は空白として残す。
const CJK_SOFT_BREAK = /(?<=\P{ASCII})\n|\n(?=\P{ASCII})/gu

/**
 * 原稿の Markdown を HTML にする。ほかのページへの相対リンクは `pagePath` で
 * サイト内の URL に変換し、外部リンクは別タブで開く。
 */
export function renderManualMarkdown(
  markdown: string,
  locale: ManualLocale,
  pagePath: (slug: string) => string,
): string {
  const joinLines = joinsLinesWithoutSpace(locale)
  const marked = new Marked({
    gfm: true,
    walkTokens(token) {
      if (joinLines && token.type === 'text' && !('tokens' in token && token.tokens)) {
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
