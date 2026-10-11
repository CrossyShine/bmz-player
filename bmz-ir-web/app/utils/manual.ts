import {
  buildManualIndex,
  manualLocaleFor,
  type ManualLocale,
  type ManualPage,
} from './manualMarkdown'

// manual/<locale>/NN-slug.md をビルド時に取り込む。原稿の規則は manual/README.md を参照。
const sources = import.meta.glob('../../../manual/*/*.md', {
  query: '?raw',
  import: 'default',
  eager: true,
}) as Record<string, string>

const pagesByLocale = buildManualIndex(sources)

/** 表示言語に対応するマニュアルの言語。原稿がなければ英語にする。 */
export function manualLocaleForSite(locale: string): ManualLocale {
  return manualLocaleFor(locale, pagesByLocale)
}

export function manualPages(locale: ManualLocale): ManualPage[] {
  return pagesByLocale[locale]
}
