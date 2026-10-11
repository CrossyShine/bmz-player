import { describe, expect, test } from 'bun:test'
import {
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
  test('Japanese and English have the same pages in the same order', () => {
    expect(pages.ja.length).toBeGreaterThan(0)
    expect(pages.en.map((page) => page.slug)).toEqual(pages.ja.map((page) => page.slug))
  })

  test('every page has a title and summary', () => {
    for (const page of [...pages.ja, ...pages.en]) {
      expect(page.title).not.toBe(page.slug)
      expect(page.summary.length).toBeGreaterThan(0)
      expect(page.markdown.startsWith('# ')).toBe(false)
    }
  })

  test('links between pages point to existing pages', () => {
    const slugs = new Set(pages.ja.map((page) => page.slug))
    for (const page of [...pages.ja, ...pages.en]) {
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
      (slug) => slug,
    )
    expect(html).toContain('操作です。キーは<strong>既定</strong>です。See\nthis.')
  })

  test('renders tables', () => {
    const html = renderManualMarkdown('| a | b |\n| --- | --- |\n| 1 | 2 |', (slug) => slug)
    expect(html).toContain('<table>')
    expect(html).toContain('<td>2</td>')
  })
})

test('locales without a manual fall back to English', () => {
  expect(manualLocaleFor('ja')).toBe('ja')
  expect(manualLocaleFor('en')).toBe('en')
  expect(manualLocaleFor('ko')).toBe('en')
  expect(manualLocaleFor('zh-TW')).toBe('en')
})
