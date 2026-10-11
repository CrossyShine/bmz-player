# 入場SEとシステムBGMの独立指定

2026-10-11、`codex/system-sound-enter-loop`で実装。
現在の契約は[システムサウンド仕様](../../docs/system-sound.md)を参照。

## 調査と判断

- `system_sound.rs`は子・親・SEセット・既定音源の候補を構成し、managerがデコード失敗時に次候補を試す。
- 通常RESULTはサウンドセットの通常名をBGM、SEセット・既定音源の通常名をSEとして別管理している。
- `.local/beatoraja/src/bms/player/beatoraja/SystemSoundManager.java`の従来名・SE探索を参照。
  BMZ独自の`.enter`を既存音種と分離し、通常名の意味とBGM優先・SEフォールバックを維持した。

## 変更

- Select、通常RESULT、COURSE RESULTの`.enter`とCOURSE専用`.loop`を音種へ追加。
  既存の音種順とSoundId割り当ては維持し、新規音種を末尾へ追加した。
- Selectでは各層の`.loop`を通常名より優先。通常RESULTのランク選択は既存判定を共有し、
  明示的入場SEとBGMを独立して選択する。COURSE専用BGMは通常RESULTから補完しない。
- Select入場待機を画面入場ごとに設定し、非同期ロード完了・同一セット再利用で一度だけ消費する。
  正規化解析、フォルダー移動、プレビュー切替では入場待機を設定しない。Viewer待機時は音声を抑制する。
- RESULT退出の停止・フェードに追加音種を含め、Select入場SEも退出・セット交換・profile切替で停止する。
- 音量区分・BGM正規化・動画出力は既存managerと音声エンジンを使用する。

## 検証

macOSの開発環境でfmt、対象crateのcheck、all-targets Clippyを実施。
新旧音源の探索、ASCII大小文字と4拡張子、破損候補からの補完、独立したランク選択、
COURSEの互換SE補完、SE二重再生防止、Select入場待機、音量区分、ミキサーでの同時再生・フェードを確認するテストを追加した。

サンドボックス内の全体テストではローカル通信を必要とする既存テストが権限エラーになった。
通信制限を外した再実行ではライブラリテスト2,344件成功、失敗0件、21件ignore。
最終差分では`cargo test -p bmz-player --locked`が成功（2,345件成功、失敗0件、21件ignore）。
`cargo fmt --check`、`cargo check -p bmz-player --locked`、
`cargo clippy -p bmz-player --all-targets --locked -- -D warnings`も成功。
生ログは`.local/system-sound-final-tests.log`等に保存し、Git管理・共有はしない。

Windows/Linuxの実行・実機音声確認、macOSのGUIでの聴感確認、GPU/動画出力のignoreテストは未実施。
外部スキン依存テストの素材欠落による早期returnもあり、テスト成功件数を全スキンの互換確認とは扱わない。
