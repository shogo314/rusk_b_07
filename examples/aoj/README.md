# AOJ の解答例

今のRuskで書いた36問・41解答です。基本入出力、番兵付きループ、配列・行列、ソート、二分探索、DPに加え、β04の関数の受け渡し・構造体付きDijkstra・セグメント木の境界探索も検証します。β05では集合、OptionによるEOF確認、多次元一括入力も使います。浮動小数点が必要な問題は見送っています。

`online-judge-tools/verification-helper` がAOJの公開テストケースを取得し、生成したC++20の実行結果を検証します。テストケースの独自コピーは収録しません。

```bash
python3 tools/test.py --suite public
oj-verify run examples/aoj/dpl_1_d.test.rusk
```

全テストの共通入口は `python3 tools/test.py` です。問題単位の `oj-verify` はPATH設定後に使えます。導入とCIの構成は [検証ガイド](../../docs/development/verification.md) を参照してください。

| 問題 | 解法・検証対象 | 解答 |
|---|---|---|
| [ALDS1_10_A](https://onlinejudge.u-aizu.ac.jp/problems/ALDS1_10_A) | 漸化式 | [alds1_10_a.test.rusk](alds1_10_a.test.rusk) |
| [ALDS1_10_B](https://onlinejudge.u-aizu.ac.jp/problems/ALDS1_10_B) | 区間DP | [alds1_10_b.test.rusk](alds1_10_b.test.rusk) |
| [ALDS1_10_C](https://onlinejudge.u-aizu.ac.jp/problems/ALDS1_10_C) | 文字列DP | [alds1_10_c.test.rusk](alds1_10_c.test.rusk) |
| [ALDS1_1_B](https://onlinejudge.u-aizu.ac.jp/problems/ALDS1_1_B) | ユークリッドの互除法 | [alds1_1_b.test.rusk](alds1_1_b.test.rusk) |
| [ALDS1_1_C](https://onlinejudge.u-aizu.ac.jp/problems/ALDS1_1_C) | 試し割り | [alds1_1_c.test.rusk](alds1_1_c.test.rusk) |
| [ALDS1_1_D](https://onlinejudge.u-aizu.ac.jp/problems/ALDS1_1_D) | 最小値の走査 | [alds1_1_d.test.rusk](alds1_1_d.test.rusk) |
| [ALDS1_2_A](https://onlinejudge.u-aizu.ac.jp/problems/ALDS1_2_A) | 隣接要素の交換 | [alds1_2_a.test.rusk](alds1_2_a.test.rusk) |
| [ALDS1_2_B](https://onlinejudge.u-aizu.ac.jp/problems/ALDS1_2_B) | 要素の交換 | [alds1_2_b.test.rusk](alds1_2_b.test.rusk) |
| [ALDS1_4_B](https://onlinejudge.u-aizu.ac.jp/problems/ALDS1_4_B) | 二分探索 | [alds1_4_b.test.rusk](alds1_4_b.test.rusk) |
| [DPL_1_A](https://onlinejudge.u-aizu.ac.jp/problems/DPL_1_A) | 最小枚数DP | [dpl_1_a.test.rusk](dpl_1_a.test.rusk) |
| [DPL_1_B](https://onlinejudge.u-aizu.ac.jp/problems/DPL_1_B) | 重さに関するDP | [dpl_1_b.test.rusk](dpl_1_b.test.rusk) |
| [DPL_1_D](https://onlinejudge.u-aizu.ac.jp/problems/DPL_1_D) | 二分探索とDP | [dpl_1_d.test.rusk](dpl_1_d.test.rusk) |
| [DPL_1_E](https://onlinejudge.u-aizu.ac.jp/problems/DPL_1_E) | 文字列DP | [dpl_1_e.test.rusk](dpl_1_e.test.rusk) |
| [ITP1_1_A](https://onlinejudge.u-aizu.ac.jp/problems/ITP1_1_A) | 入出力 | [itp1_1_a.test.rusk](itp1_1_a.test.rusk) |
| [ITP1_1_B](https://onlinejudge.u-aizu.ac.jp/problems/ITP1_1_B) | 整数演算 | [itp1_1_b.test.rusk](itp1_1_b.test.rusk) |
| [ITP1_1_C](https://onlinejudge.u-aizu.ac.jp/problems/ITP1_1_C) | 整数演算 | [itp1_1_c.test.rusk](itp1_1_c.test.rusk) |
| [ITP1_1_D](https://onlinejudge.u-aizu.ac.jp/problems/ITP1_1_D) | 文字列と整数演算 | [itp1_1_d.test.rusk](itp1_1_d.test.rusk) |
| [ITP1_2_A](https://onlinejudge.u-aizu.ac.jp/problems/ITP1_2_A) | 条件分岐 | [itp1_2_a.test.rusk](itp1_2_a.test.rusk) |
| [ITP1_2_B](https://onlinejudge.u-aizu.ac.jp/problems/ITP1_2_B) | 比較と論理演算 | [itp1_2_b.test.rusk](itp1_2_b.test.rusk) |
| [ITP1_2_C](https://onlinejudge.u-aizu.ac.jp/problems/ITP1_2_C) | ソート | [itp1_2_c.test.rusk](itp1_2_c.test.rusk) |
| [ITP1_2_D](https://onlinejudge.u-aizu.ac.jp/problems/ITP1_2_D) | 比較と論理演算 | [itp1_2_d.test.rusk](itp1_2_d.test.rusk) |
| [ITP1_3_B](https://onlinejudge.u-aizu.ac.jp/problems/ITP1_3_B) | 番兵とループ | [itp1_3_b.test.rusk](itp1_3_b.test.rusk) |
| [ITP1_3_C](https://onlinejudge.u-aizu.ac.jp/problems/ITP1_3_C) | 番兵とループ | [itp1_3_c.test.rusk](itp1_3_c.test.rusk) |
| [ITP1_3_D](https://onlinejudge.u-aizu.ac.jp/problems/ITP1_3_D) | 全探索 | [itp1_3_d.test.rusk](itp1_3_d.test.rusk) |
| [ITP1_4_D](https://onlinejudge.u-aizu.ac.jp/problems/ITP1_4_D) | 集計 | [itp1_4_d.test.rusk](itp1_4_d.test.rusk) |
| [ITP1_6_A](https://onlinejudge.u-aizu.ac.jp/problems/ITP1_6_A) | 反転 | [itp1_6_a.test.rusk](itp1_6_a.test.rusk) |
| [ITP1_6_D](https://onlinejudge.u-aizu.ac.jp/problems/ITP1_6_D) | 多次元配列 | [itp1_6_d.test.rusk](itp1_6_d.test.rusk) |
| [ITP1_7_A](https://onlinejudge.u-aizu.ac.jp/problems/ITP1_7_A) | 関数と条件分岐 | [itp1_7_a.test.rusk](itp1_7_a.test.rusk) |
| [ITP1_7_B](https://onlinejudge.u-aizu.ac.jp/problems/ITP1_7_B) | 全探索 | [itp1_7_b.test.rusk](itp1_7_b.test.rusk) |
| [ITP1_7_C](https://onlinejudge.u-aizu.ac.jp/problems/ITP1_7_C) | 行列の集計 | [itp1_7_c.test.rusk](itp1_7_c.test.rusk) |
| [ALDS1_2_C](https://onlinejudge.u-aizu.ac.jp/problems/ALDS1_2_C) | キー付き安定ソート・map | [alds1_2_c.test.rusk](alds1_2_c.test.rusk) |
| [ALDS1_4_D](https://onlinejudge.u-aizu.ac.jp/problems/ALDS1_4_D) | 捕捉した条件関数とfirst_true | [alds1_4_d.test.rusk](alds1_4_d.test.rusk) |
| [DSL_3_A](https://onlinejudge.u-aizu.ac.jp/problems/DSL_3_A) | 汎用セグメント木のmax_right | [dsl_3_a.test.rusk](dsl_3_a.test.rusk) |
| [DSL_3_A](https://onlinejudge.u-aizu.ac.jp/problems/DSL_3_A) | min_leftによる別解 | [dsl_3_a_min_left.test.rusk](dsl_3_a_min_left.test.rusk) |
| [GRL_1_A](https://onlinejudge.u-aizu.ac.jp/problems/GRL_1_A) | 構造体と比較関数付き優先度キューによるDijkstra | [grl_1_a.test.rusk](grl_1_a.test.rusk) |
| [ITP1_4_D](https://onlinejudge.u-aizu.ac.jp/problems/ITP1_4_D) | タプルを蓄積するfoldの別解 | [itp1_4_d_fold.test.rusk](itp1_4_d_fold.test.rusk) |
| [ALDS1_4_C](https://onlinejudge.u-aizu.ac.jp/problems/ALDS1_4_C) | 文字列のSetとif式 | [alds1_4_c.test.rusk](alds1_4_c.test.rusk) |
| [ITP1_6_D](https://onlinejudge.u-aizu.ac.jp/problems/ITP1_6_D) | 多次元一括入力の別解 | [itp1_6_d_bulk.test.rusk](itp1_6_d_bulk.test.rusk) |
| [ITP1_3_C](https://onlinejudge.u-aizu.ac.jp/problems/ITP1_3_C) | try_read・ブロック式・ラベル付きbreak・タプルのif式 | [itp1_3_c_option.test.rusk](itp1_3_c_option.test.rusk) |

β06では[ITP1_3_Cのパターン分岐版](itp1_3_c_patterns.test.rusk)と、[ITP1_8_B](https://onlinejudge.u-aizu.ac.jp/problems/ITP1_8_B)の[解答](itp1_8_b.test.rusk)を追加しました。while let・タプル/文字列パターン・ガードを公開ケースで検証します。

[DPL_1_A](dpl_1_a.test.rusk)のDP更新は`<?=`、[ALDS1_1_D](alds1_1_d.test.rusk)の走査は`<?=`・`>?=`を使います。
