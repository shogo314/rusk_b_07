# テストの実行

テストの共通入口は `tools/test.py` です。リポジトリのルートで次を実行すると、fmt・clippy・CargoのE2EとAtCoderサンプル・最適化無効版の比較・サニタイザー・AOJとLibrary Checkerの公開テストを順に確認し、最後に全段階の成否を一覧表示します。一つでも失敗すると終了コード1を返します。

```bash
python3 tools/test.py
```

公開テストは [online-judge-tools/verification-helper](https://github.com/online-judge-tools/verification-helper) に任せます。`oj download --system` でケースを取得し、`oj test` で実行結果を検査します。Library Checkerでは公式のチェッカーも利用します。AtCoderの100問はCargoテストから保存済み公式サンプル279件と照合します。

## 実行対象を絞る

| コマンド | 対象 |
|---|---|
| `python3 tools/test.py` | 全段階 |
| `python3 tools/test.py --suite local` | fmt・clippy・E2E・AtCoderサンプルと最適化無効版の回帰検証。公開テストの取得は不要 |
| `python3 tools/test.py --suite sanitizers` | ASan・UBSan・LeakSanitizer |
| `python3 tools/test.py --suite public` | AOJ・Library Checkerの公開テスト |

ptrace環境でLeakSanitizerが動かない場合は `--no-leaks` を付けます。ASanとUBSanは引き続き有効です。

各段階の失敗後も独立した残りの段階を実行します。ビルドに失敗した場合は、そのビルドに依存する実行を止め、古いバイナリを検証してしまうことを防ぎます。必須ツールがない段階も失敗として表示します。

## 導入と実行

Rust stable、Python 3.12以降、Git、GCCのC++20環境が必要です。以下はリポジトリのルートで実行します。

```bash
python3 -m venv .venv-verify
.venv-verify/bin/python -m pip install -r requirements-verify.txt
python3 tools/test.py
```

OSによっては `python3-venv` パッケージが必要です。依存関係は `requirements-verify.txt` に記録しています。verification-helper 5.6.0は `pkg_resources` を使うため、これを含む `setuptools<81` を指定しています。

検証対象を明示すると、処理系自身のRustソースや `target/` を自動探索する処理を避けられます。問題単位で調べる場合はverification-helperを直接実行できます。共通入口は `.venv-verify/bin` を自動的にPATHへ加えますが、直接使う場合は次のように設定します。

```bash
export PATH="$PWD/.venv-verify/bin:$PATH"
oj-verify run examples/aoj/alds1_1_b.test.rusk
oj-verify run examples/library_checker/unionfind.test.rusk
oj-verify run examples/aoj/*.test.rusk
oj-verify run examples/library_checker/*.test.rusk
```

初回はインターネット接続が必要です。AOJは公開APIからテストケースを取得し、Library Checkerは問題リポジトリを取得してジェネレーターと参照解答からテストケースを生成します。大きな入力も含まれるため、初回の取得・生成には時間とディスク容量を使います。

## Ruskとの接続

`.verify-helper/config.toml` でRuskをカスタム言語として登録しています。対象は `*.test.rusk` で、先頭のコメントに問題URLを書きます。

```text
// verification-helper: PROBLEM https://judge.yosupo.jp/problem/aplusb
in a,b:Int;
wt a+b;
```

コンパイルは `tools/verify_rusk.py` が次の順で行います。

1. `cargo build --locked --release` でコンパイラを用意する。
2. RuskソースをC++20へ変換する。
3. `g++ -std=c++20 -O2` で実行ファイルを作る。

実行と正誤判定はverification-helperとojに任せます。問題ソースに加えて、`include` の依存ファイルに加え、コンパイラのRustソース、文法、標準ライブラリ、ビルド設定、検証用スクリプトも依存ファイルとして登録するため、処理系の変更後は再検証されます。

## キャッシュと結果

`.verify-helper/cache/` に問題ごとのテストケース・生成C++・実行ファイルが置かれ、`.verify-helper/timestamps.local.json` に成功履歴が保存されます。どちらもGitへ追加しません。依存ファイルが変わらなければ成功済みの検証を省略します。再実行を強制したいときは成功履歴ファイルを削除します。

GitHub Actionsの `Test` も `python3 tools/test.py` を使い、取得済み公開テストをキャッシュします。共通入口は `examples/` 以下の `*.test.rusk` を自動で列挙します。解答を追加すると、手元とCIの両方で対象になります。

この検証は公開ケースに対するローカル実行です。オンラインジャッジへの提出・AC確認とは区別します。`oj-verify` の既定の実行時間制限は1ケース60秒で、各問題のジャッジ環境の制限時間を再現するものではありません。`--tle` で変更できます。

## 最適化とバッファ入出力の検証

`tests/optimization.rs` は共有・評価順序・終了コード・エラー位置を通常版と `--no-opt` で比較し、アロケーターを計測して一時値の確保削減も確認します。`tests/buffered_io.rs` は64 KiB境界をまたぐ入力・出力、CRLF・EOF・NUL、トークンと行入力の混在、外部のiostreamとの接続、不正入力を確認します。

共通入口ではE2E・構造体・タプル・ジェネリクス・include・データ構造・関数の受け渡しに加え、後述の契約検査・診断・文書例も最適化無効で実行します。対象の一覧は`tools/test.py`で管理します。性能の測定手順と結果は [β03の高速化と検証](beta03.md) に記録しています。

β04の `tests/callables.rs`・`tests/higher_order.rs`・`tests/beta04_structures.rs` では捕捉の共有とコピー、寿命、遅延評価、短絡、安定性、キーの評価回数、コールバック中の構造変更を確認します。境界探索は180回の更新と素朴な走査、優先度キューは整列した多重集合を独立した基準として照合します。

β05の `tests/beta05.rs` はif式・Option・不在演算子・比較の連鎖・Dict/Set・構造化入力を通常版と最適化無効版で検証します。辞書・集合は240回の変更を独立した標準コンテナと照合します。サニタイザーにも新しい共有と寿命の例を加えています。[β05の実装と検証](beta05.md)を参照してください。

`tests/pipeline.rs` ではCST/AST/HIR/LIR/C++の出力、最適化によらないIRの一致、解決済みの型・参照先、ジャンプをまたぐ共有の解放と返却値の寿命を確認します。内部テストでは不正なIRの検出と、外側のループへのジャンプの解放、合流時に残る共有の解析を検証します。[パイプライン](pipeline.md)を参照してください。

`tests/blocks_labels.rs`はブロック式の結果と共有、ラベルの名前解決と範囲、脱出する分岐、短絡、クロージャの捕捉・戻り型、文字への共有の寿命を検証します。`examples/beta05/blocks_and_labels.rusk`はサニタイザーにも含めます。

`tests/enums.rs`は型引数付きenum、網羅的match、payloadの共有・コピーと順序、variant切替、外側への制御移動、Optionの互換性を検証します。最適化無効版にも含め、enum導入時のサニタイザーは、enumの2例を加えて計15例でした。[enumとOption移行](enums.md)を参照してください。

β06では`tests/beta06.rs`を通常版・最適化無効版の共通入口に追加しています。256通りのarm集合と独立な有限値列挙の照合、ガードの評価・結び直し・寿命、捕捉、if let・while let、公開解答のサンプルを検査します。β06のこの段階でサニタイザーは計20例でした。[β06の検証記録](beta06.md)を参照してください。

最小・最大演算子の`tests/minmax.rs`も通常版・最適化無効版に含めます。公開DP・最大利益の解答にも新しい更新演算子を使います。

## β07の契約・最適化・文書例の検証

`tests/effects.rs`と`tests/pipeline.rs`は効果と返却する共有元がHIR/LIRに残ることを確認します。`tests/bindings.rs`・`tests/readonly.rs`・`tests/noescape.rs`は、注釈の省略・両側の指定、初期化・結び直し・共有・捕捉・呼び出し・返却の契約を扱います。`tests/heap_aliases.rs`は格納先の別名や合流と保守的な制限を検証します。これらは最適化無効版でも実行します。

`tests/readonly_routes.rs`・`tests/noescape_routes.rs`は宣言・別名・格納・呼び出し先内部の経路、不明な共有と追跡上限を検証します。HIR/LIR/C++の各出力段階で診断が一致し、拒否時に出力を作らないことも確認します。

`tests/effect_optimization.rs`は生成C++の借用・所有・値保持の選択を調べ、実行結果・共有更新・文字参照の解放・失敗時の診断を最適化無効版と比較します。サニタイザーの対象は`tools/check_sanitizers.py`の`CASES`が正で、β07の実装完了時点では35例です。各変更時点の件数や性能測定は[β07の実装と検証](beta07.md)に記録しています。

`tests/documentation.rs`はfixed・readonly・noescapeのリファレンスからコード例を直接読み取ります。必要な呼び出しを補ってジェネリック関数やコールバックも具体化し、通常版・最適化無効版で実行結果を検査します。コメントで示した違反や、本文で説明した保守的な拒否も有効にして検査し、診断例の経路表示を実際の出力と照合します。すべてのドキュメントの構文断片を自動検証する機能ではありません。
