import { describe, expect, test } from 'bun:test'
import {
  MANUAL_LOCALES,
  MANUAL_PAGE_LINK_PATTERN,
  buildManualIndex,
  manualLocaleFor,
  renderManualMarkdown,
} from '../app/utils/manualMarkdown'

const sources: Record<string, string> = {}
for await (const path of new Bun.Glob('manual/*/*.md').scan({ cwd: '.' })) {
  sources[`/${path}`] = await Bun.file(path).text()
}
const pages = buildManualIndex(sources)

describe('manual pages', () => {
  test('every locale has the same pages in the same order', () => {
    expect(pages.ja.length).toBeGreaterThan(0)
    for (const locale of MANUAL_LOCALES) {
      expect(pages[locale].map((page) => page.slug)).toEqual(pages.ja.map((page) => page.slug))
    }
  })

  test('every page has a title and summary', () => {
    for (const page of MANUAL_LOCALES.flatMap((locale) => pages[locale])) {
      expect(page.title).not.toBe(page.slug)
      expect(page.summary.length).toBeGreaterThan(0)
      expect(page.markdown.startsWith('# ')).toBe(false)
    }
  })

  test('links between pages point to existing pages', () => {
    const slugs = new Set(pages.ja.map((page) => page.slug))
    for (const page of MANUAL_LOCALES.flatMap((locale) => pages[locale])) {
      for (const match of page.markdown.matchAll(/\]\((?<href>[^)]+)\)/gu)) {
        const href = match.groups!.href!
        if (/^[a-z]+:/u.test(href)) continue
        const slug = MANUAL_PAGE_LINK_PATTERN.exec(href)?.groups?.slug
        expect(slug ? slugs.has(slug) : `${page.slug}: ${href}`).toBe(true)
      }
    }
  })
})

describe('renderManualMarkdown', () => {
  test('rewrites page links and opens external links in a new tab', () => {
    const html = renderManualMarkdown(
      'See [Setup](02-setup.md#keys) and [Releases](https://example.com/?a=1&b=2).',
      'en',
      (slug) => `/en/manual/${slug}`,
    )
    expect(html).toContain('<a href="/en/manual/setup#keys" data-manual-link>Setup</a>')
    expect(html).toContain(
      '<a href="https://example.com/?a=1&amp;b=2" target="_blank" rel="noopener noreferrer">Releases</a>',
    )
  })

  test('joins Japanese lines without inserting a space', () => {
    const html = renderManualMarkdown(
      '操作です。\nキーは**既定**です。\nSee\nthis.',
      'ja',
      (slug) => slug,
    )
    expect(html).toContain('操作です。キーは<strong>既定</strong>です。See\nthis.')
  })

  test('keeps line breaks as spaces in Korean and joins them in Chinese', () => {
    expect(renderManualMarkdown('감사합니다.\n이 페이지', 'ko', (slug) => slug)).toContain(
      '감사합니다.\n이 페이지',
    )
    expect(renderManualMarkdown('感谢下载。\n本页介绍', 'zh-CN', (slug) => slug)).toContain(
      '感谢下载。本页介绍',
    )
  })

  test('renders tables', () => {
    const html = renderManualMarkdown('| a | b |\n| --- | --- |\n| 1 | 2 |', 'en', (slug) => slug)
    expect(html).toContain('<table>')
    expect(html).toContain('<td>2</td>')
  })
})

test('each site locale uses its own manual and unknown locales fall back to English', () => {
  for (const locale of MANUAL_LOCALES) {
    expect(manualLocaleFor(locale, pages)).toBe(locale)
  }
  expect(manualLocaleFor('fr', pages)).toBe('en')
  expect(manualLocaleFor('ko', { ...pages, ko: [] })).toBe('en')
})
