# enumとOption移行の実装

β05への追加要件として、型引数付きenum、variantの構築、網羅的なmatch、共有・コピー・代入を実装した。利用者向けの文法と制限は[enumとmatch](../reference/enums.md)を参照する。

## 各段階の責務

- CSTはenum定義、修飾付き参照、matchと各armの構造を保持する。
- ASTはenumの型引数・variantごとのpayload型と、matchの対象・パターン・本体を保持する。
- 型検査でenumを具体化し、variantの引数と型引数を検査・推論する。matchは対象型からvariantを解決し、重複・網羅性・payloadの数・束縛の重複・続行するarmの型一致を検査する。
- HIRの型表は汎用の`Enum`型記述を持つ。構築は`Variant`、タグ検査は`VariantTest`、取り出しは`VariantField`で表す。パターンの変数は通常の一意なValueIdを持つ。
- matchは共通の構造化分岐処理へ下げ、LIRで基本ブロックとジャンプになる。armのスコープを抜けるreturn・break・continueの解放には既存の経路を使う。
- C++バックエンドはvariantごとのpayload構造体を`std::variant`で格納する。外側は通常の共有オブジェクトで、payload構造体のための追加のヒープ確保はしない。

対象を一度だけ評価し、armの本体を実行する前に必要なpayloadを共有で束縛する。対象のenumが別variantへ更新されても、束縛済みの値は有効である。

## コピー

各payloadのコピー先はRuskの論理型で解決する。同じC++ネイティブ型に対応する別のRusk型でも、固有のcopyフックを使う。variantの格納値を共有でスナップショットしてから、payloadを宣言順にコピーする。C++の関数引数の評価順には依存しない。全体への代入ではコピーが完了してから格納値を移し、外側の共有先を保つ。

循環した型はenum・構造体・コンテナをまたいでも拒否する。循環検査では検査済みの型を記憶し、深い有限の入れ子を繰り返し探索しない。具体化数・深さ・型名の展開量にも上限を設ける。

## Optionの移行

標準ライブラリーの`lib/collections.rusk`に`enum Option<T> { Some(T), None, }`を定義する。Option専用のHIR型・存在検査命令・値取得命令と、C++の`RuskOption`は削除した。コンテナとしてのOption分類も持たない。

短いSome/None構築記法、Optionの既定値、存在検査・真偽値変換・位置付き取得は互換APIとして残す。これらとget・find・try_readは、汎用enumと同じタグ・payload・構築ヘルパーを使う。`??`は汎用variant検査と取り出しを共通の分岐処理に接続する。

## 検証

`tests/enums.rs`は、構築・型引数推論、共有とコピー、variant切替、タプル分解、捕捉、複合型への格納、評価順、外側への制御移動、Option互換、位置付きエラー、中間表現を検査する。深い有限型の検査と、論理型ごとのコピー・payloadのコピー順も回帰テストに含める。

統合ランナー`python3 tools/test.py`で通常版と`--no-opt`の両方を実行する。サニタイザーにもenumの実行例と、文字参照・配列payloadがvariant変更後に生存するケースを追加した。CIは同じ統合ランナーを使う。

最終検証では通常244テスト・最適化無効130テスト・ASan/UBSan/LeakSanitizerの15例・公開49解答805ケースが成功した。enum専用の13テストは両バックエンドで実行し、文書の完全なプログラム例5件も変換・実行した。書式検査、Clippy、MkDocsのstrictビルドも成功した。CIはpush後に同じ一式を実行する。

## 性能測定

移行前のβ05コンパイラーと同じ入力を交互に実行し、7回の中央値を比較する。既存10項目に、OptionのSome/None生成と`??`を100万回繰り返す項目を加えた。最適化無効版・手書きC++・PyPyとも出力一致を検査する。変換・コンパイル時間は測定に含めない。

```sh
python3 tools/benchmark.py --compiler target/release/rusk \
  --baseline-compiler /tmp/rusk-before-enums --baseline-all-cases \
  --compare-no-opt --repeats 7 --output docs/development/enums-benchmark.json
```

`/tmp/rusk-before-enums`は移行前に保存したバイナリーで、再測定時は対応する旧版を用意する。[測定JSON](enums-benchmark.json)には処理系・ソース・入力のハッシュ、環境、各回の時間を保存する。測定結果はこの入力と環境での値であり、一般のプログラムの速度を保証しない。

| 処理 | 移行前 | enum移行後 | 時間の増減 |
|---|---:|---:|---:|
| option | 39.73 ms | 43.34 ms | +9.1% |
| dictionary | 13.12 ms | 13.38 ms | +2.0% |
| tuple_set | 21.22 ms | 20.94 ms | -1.3% |
| callback_fold | 87.45 ms | 86.83 ms | -0.7% |
| callback_sort | 26.37 ms | 25.31 ms | -4.0% |
| integer_input | 29.21 ms | 29.41 ms | +0.7% |
| integer_output | 5.36 ms | 4.98 ms | -7.2% |
| fenwick | 39.94 ms | 42.36 ms | +6.0% |
| range_sum | 14.61 ms | 13.96 ms | -4.5% |
| sort_search | 13.59 ms | 14.33 ms | +5.5% |
| grid_bfs | 28.82 ms | 29.07 ms | +0.9% |

既存10項目は約−7〜＋6%の範囲だった。Optionを大量に生成する項目は39.73 msから43.34 msへ約9%遅くなった。汎用表現への移行に伴う退行として残し、最適化の課題とする。Optionの大量生成の結果を、全体の処理が9%遅くなるという意味には扱わない。
