# 設定画面からのIR一括操作と送信キュー画面

実施日: 2026-10-11。対象commit: `aeec4552`, `86fd4dc7`, `ee1ee82f`, `576b09c5` と、この記録を含む文書commit。
現在の仕様は [docs/ir.md](../../docs/ir.md) と [docs/controls.md](../../docs/controls.md)、
プレイヤー向けの説明は [manual/ja/02-setup.md](../../manual/ja/02-setup.md) を参照してください。

## 目的

`bmz ir upload-local --all` と `bmz ir download-scores` 相当の操作、およびIR送信キュー
（`network.db` の `ir_score_jobs`）の確認・再送を、ゲーム内の設定ウィンドウから行えるようにする。

## 経緯（5フェーズ）

1. **CLI処理の分離**（`aeec4552`）: `ir_cmd` の処理と `println!` が混在していたため、出力しない
   `ir::manual` に移した。進捗は `IrManualProgress` のコールバック、中断は `watch::Receiver<bool>`、
   停止理由は `IrManualStop`（完了・中断・失敗job停止・sending待ち）で返す。CLIは進捗通知を
   従来の出力へ変換する薄い層（`ir_cmd/output.rs`）にし、出力文言と終了コードは変えていない。
2. **キューAPI**（`86fd4dc7`）: `list_ir_score_jobs` / `count_ir_score_jobs` /
   `ir_score_job_status_counts`、`resend_job` / `resend_failed_jobs`。
3. **appの手動IR操作ジョブ**（`ee1ee82f`）: `ir::manual_job::IrManualHandle` とapp側の接続。
4. **設定UI**（`576b09c5`）: IRページの「スコアの一括操作」、確認モーダル、送信キューウィンドウ。
5. **文書**: docs・マニュアル（6言語）・この記録。

## 設計判断

- **送信間隔**: 手動操作はCLIと同じ `IR_CLI_SYNC_BATCH_LIMIT`（100）/ `IR_CLI_SYNC_JOB_SPACING_MS`（200ms）。
  常駐ワーカー（20件・3.1秒）とは別の予算で、手動検証用の速い設定をそのまま使う。
- **`--all` 相当の完走**: GUIでは1回の操作で候補が尽きるまで進める。`include_replay` /
  `include_course_stages` はCLI既定値に固定した。スコア取得はCLI既定の200件上限を付けず全件を対象にした
  （重複はDB側で除外されるので、中断後の再実行は続きから進む）。
- **再送はSQLで状態を書き換えない**: 既存のclaim（`ignore_retry_backoff = true`）経由にした。
  claimは `Immediate` トランザクション内で `sending` へ更新するので、常駐ワーカーが送信中のjobを
  二重に送らない。claim APIはbackoff中のfailedを拾えたため変更していない。
- **current-thread runtimeの専用スレッド**: `download_ir_scores` のfutureは `&NetworkDatabase` を
  await越しに保持して `Send` にならない。そのため手動操作は専用スレッドで current-thread runtime を
  `block_on` する。他のバックグラウンド処理（std thread + mpsc）とも揃う。
- **常駐ワーカーの一時停止**: `select_rx`（Select中だけ動かす）とは別の `pause_rx` を追加した。
  二重claimは起きないが、手動操作の進捗や送信間隔に常駐ワーカーの送信が混ざるのを避ける。
  解除は `Drop` guardで行い、panicしても再開する。
- **lease待ち**: 他の主体が `sending` のjobを持つと進捗が出ない。GUIではCLIのように即エラーにせず、
  15秒間隔で再試行し、進捗なしが6分（lease 5分 + 余裕）続いたら停止して報告する。待機中も中断できる。
  失敗jobがあった場合は待たずに停止する。
- **世代番号**: 書き込み系・件数見積もり・一覧読み込みを別々の世代で管理し、要求時点の世代を
  要求に持たせる。dispatch時に最新世代を読むと、古い要求が新しい結果として反映されうるため
  （テストで判明して修正）。
- **終了時**: 中断を要求して処理中の1件を最大3秒待つ。待ち切れなくてもjobは `sending` のまま残り、
  5分のleaseで再取得される。DBは壊れない。
- **profile切替**: 実行中は切替を拒否する（`profile_change_allowed`）。切替時は状態を破棄する。

## 検証結果

- 各フェーズ後に `cargo fmt --check`、`cargo clippy -p bmz-player --all-targets --locked -- -D warnings`、
  `cargo test -p bmz-player --locked` が通った（Phase 4後は2369 passed、21 ignored）。
- テストした範囲: 進捗通知・中断・sending中jobのスキップ・進捗なし判定、failedの再送対象化と二重claimの防止、
  一覧のfilter・順序・ページング、世代破棄、中断、常駐ワーカーの一時停止と再開、UIのsmoke描画、
  全ロケールのi18nキー・パラメータ整合。
- 通信を伴う確認は、未設定providerのjobをローカルで失敗させる方法で行った。
- ダミーprovider（`127.0.0.1:1`）と失敗jobを入れた隔離データ（`BMZ_DATA_DIR` 等を一時ディレクトリへ）で、
  アプリが起動してpanicしないことを確認した。

## 未実施の確認

- **UIの目視**: スクリーンキャプチャが使えず、IRページ・確認モーダル・送信キューウィンドウの
  見た目と操作は確認していない。
- **実IRへの送信**: 一括送信・スコア取得・再送とも、実際のIRサーバーでは確認していない。
- 韓国語・中国語（zh-CN / zh-TW / zh-HK）の文言は、ネイティブによる確認をしていない。
- Windows / Linux での動作確認。

## 気づいた点

- 設定画面の `ir-bulk-no-provider` は、zh-CN で「设为主要 IR」と書いているが、同じ画面の
  項目名（`profile-ir-primary`）は「主 IR」。表記を揃える余地がある。
