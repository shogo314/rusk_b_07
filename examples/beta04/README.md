# β04の実例

`library/priority_queue.rusk` は比較関数を指定する優先度キュー、`library/segment_tree.rusk` は演算を指定するセグメント木です。後者は `max_right` / `min_left` による境界探索にも対応します。

| 解答例 | 利用する機能 |
|---|---|
| `../aoj/grl_1_a.test.rusk` | 構造体の状態と比較関数付き優先度キューによるDijkstra |
| `../library_checker/point_set_range_composite.test.rusk` | 一次関数の合成を演算として渡すセグメント木 |
| `../aoj/dsl_3_a.test.rusk` | max_rightによる区間和の境界探索 |
| `../aoj/dsl_3_a_min_left.test.rusk` | min_leftを使う別解 |
| `../aoj/alds1_2_c.test.rusk` | 構造体のキー付き安定ソートとmap |
| `../aoj/itp1_4_d_fold.test.rusk` | タプルを蓄積値にしたfold |
| `../aoj/alds1_4_d.test.rusk` | 捕捉付きの条件関数を渡す二分探索 |

共通の `python3 tools/test.py` から、保存したAtCoderサンプル・Cargoの回帰テストと、これらの公開テストを確認できます。APIと前提条件は `docs/library/beta04.md`、捕捉の意味は `docs/reference/beta04.md` を参照してください。
