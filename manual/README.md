# BMZ Player マニュアル原稿

このディレクトリは、プレイヤー向けマニュアルの原稿です。
BMZ IR の `/manual` で公開し、ゲーム内の F1 メニューからも開けます。

- 日本語: [ja/](ja/01-introduction.md)
- English: [en/](en/01-introduction.md)

## ファイルの規則

- `manual/<言語>/NN-slug.md` の形で置きます。`NN` は目次の並び順、`slug` は URL に使います。
- 各ファイルの最初の `# 見出し` がページタイトルになります。
- ほかのページへのリンクは `02-setup.md` のように相対パスで書きます。IR では `/manual/setup` へ変換されます。
- 日本語と英語で、同じ `NN-slug.md` のファイルをそろえます。
- 韓国語・中国語の利用者には英語版を表示します。

文章の書き方は [docs/writing-style.md](../docs/writing-style.md) に従います。
