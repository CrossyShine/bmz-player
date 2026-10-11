# BMZ Player マニュアル原稿

このディレクトリは、プレイヤー向けマニュアルの原稿です。
BMZ IR の `/manual` で公開し、ゲーム内の F1 メニューからも開けます。

- 日本語: [ja/](ja/01-introduction.md)
- English: [en/](en/01-introduction.md)
- 한국어: [ko/](ko/01-introduction.md)
- 简体中文: [zh-CN/](zh-CN/01-introduction.md)
- 繁體中文（台灣）: [zh-TW/](zh-TW/01-introduction.md)
- 繁體中文（香港）: [zh-HK/](zh-HK/01-introduction.md)

## ファイルの規則

- `manual/<言語>/NN-slug.md` の形で置きます。`NN` は目次の並び順、`slug` は URL に使います。
- 各ファイルの最初の `# 見出し` がページタイトルになります。
- ほかのページへのリンクは `02-setup.md` のように相対パスで書きます。IR では `/manual/setup` へ変換されます。
- すべての言語で、同じ `NN-slug.md` のファイルをそろえます。テスト（`bmz-ir-web/tests/manual.test.ts`）で確認します。
- 内容を変えるときは、日本語版を基準に、全言語を同じ変更で更新します。
- 画面の項目名は、各言語のゲーム内カタログ（`crates/bmz-player/src/i18n/locales/`）の表記に合わせます。
- `zh-HK` は `zh-TW` を基に、香港の用語（スキン＝介面、アナログ＝模擬など）とゲーム内の `zh-HK` の表記へ置き換えて作ります。
- 原稿のない言語が追加された場合、IR では英語版を表示します。

文章の書き方は [docs/writing-style.md](../docs/writing-style.md) に従います。
