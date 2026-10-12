# upload-local / download-scores レビューと修正

2026-10-12に、CLIから手動IR処理、非同期GUI通知、score.db / network.dbの保存まで確認した。
対象は `upload-local` と `download-scores`。Luna（gpt-6-luna）が1件ずつ実装し、
親エージェントが差分をレビューし、検証後に順番にcommitした。
レビュー開始時の基点は `e10d45f7`。

現在の仕様は [IR](../../docs/ir.md) と
[スコア保存](../../docs/score-persistence.md)を参照する。
GUI一括操作の導入経緯は [前日の記録](2026-10-11-ir-settings-bulk-ops.md)にある。

## 指摘と対応

| # | 問題と影響 | 対応 | commit |
|---|---|---|---|
| 1 | 成功jobのpruningでsubmissionも消え、既送信判定とremote score IDからのリンクを失う | jobと独立した成功台帳を追加。upload判定、downloadリンク、attestation、cleanupが参照する | `a39f202e` |
| 2 | IRから取得した履歴を同じprovider/accountへ再アップロードする | source markerとprovider/accountを照合して候補から除外。`--resend`でも除外する | `bbceb75f` |
| 3 | local history IDを使う送信キーが別profileで衝突する | 履歴ごとのUUIDを永続化。再起動・再送で維持し、複製後の新規履歴は別キーにする | `5df84b67` |
| 4 | 途中FAILEDのbackfillが実際の通過ノーツ数を失い、download時のBP/CB検証で弾かれる | 判定数からpass_notesを保存。旧payloadはFAILED・BP/CB差分・判定数が整合する場合だけ補正する | `ad54a378` |
| 5 | `upload-local --sync/--all` が別provider/accountやreplay/courseも送信する | 選択したprovider/accountのscore jobへ同期範囲を限定。空batchは正常終了する | `c29ea954` |
| 6 | 成功jobへの再enqueueがno-opなのに投入件数へ加算され、全件再送も同じ履歴を繰り返す | 成功時の本文を保存し、同じキー・本文で再送。pending/sendingを保護し、実行内の処理済み履歴を追跡する | `12deeb01` |
| 7 | GUIの取得完了後もSelect・フォルダ集計・player statsが古い | 新規保存を伴う通知で再読込。中断・エラー時の保存済み分も対象にする | `f3b9cf36` |
| 8 | 行単位の取得失敗がCLI/GUIで成功扱いになる | CLIを非ゼロ終了にし、GUIで部分失敗と取込・紐付け・失敗件数を表示する | 本記録を含むcommit |

主な入口は `crates/bmz-player/src/` 以下の `ir/backfill.rs`、`ir/download.rs`、
`ir/manual/upload.rs`、`ir/manual_job.rs`、`ir_cmd/download.rs`、
`app/background_jobs/ir_manual.rs`、`storage/network_db/operations.rs`。

## DB移行と保持するデータ

- score.db migration 32は履歴のIR識別キーを追加する。既存スコア値やBESTを変えない。
- network.db migration 5は成功台帳を作り、残っている成功submissionを移す。
  移行前にpruningで失われたremote IDや紐付けは復元できない。
- network.db migration 6は成功時の元payloadとchart hash / LN policyを台帳に保持する。
  jobは従来どおりpruningするが、台帳の容量は成功履歴数と本文サイズに応じて増える。
  cleanupでは対応する台帳とpayloadも削除する。
- 元payloadがすでに消去された旧成功履歴は安全に再構築できないため、
  再送不可の件数として報告する。新しいキーで別スコアとして作り直さない。
- 旧版へ戻す際は書き込み停止中に採った整合したDB backupへ復元する。
  schema versionだけの巻き戻しは行わない。

## 検証

macOSのローカル環境で、各commit前に以下を確認した。

```bash
cargo fmt --check
cargo check -p bmz-player --locked
cargo clippy -p bmz-player --all-targets --locked -- -D warnings
cargo test -p bmz-player --locked
```

回帰テストは一時DBとローカルHTTP mockを使用する。主な確認対象は、
pruning後の重複防止、取得元への再送除外、別DBのキー非衝突と再open時の維持、
FAILEDの往復、同期先の限定、元payloadを保つ再送、dry-runの不変性、
limit 1での全件再送の有限終了、取込通知とprofile世代の扱い。
最終修正では正常・不正データ混在時の保存と再実行、CLIの失敗判定、
部分失敗の結果分類、6言語の通知件数とカタログ整合性も確認した。

最終状態でfmt、check、all-targets Clippyは成功。
bmz-player全テストは2390件成功、0件失敗、21件ignore。
生ログは `.local/performance/ir-bulk-fixes/` に置き、Gitには含めない。

実IRへの送受信、実際のGUI操作、Windows / Linux実機での確認は未実施。
GPUや外部アセットを必要とする既存テストのignore・早期returnを、
今回の画面表示やスキン互換性の実機確認済みとは扱わない。
