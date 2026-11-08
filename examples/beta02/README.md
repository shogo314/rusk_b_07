# β02 の自作ライブラリ

構造体に状態をまとめ、型引数を使い、`include` で別の解答から再利用する実例です。

| ファイル | 操作 | 必要な性質 |
|---|---|---|
| [union_find.rusk](library/union_find.rusk) | `UnionFind(n)`、`unite`、`same`、`leader` | 頂点番号は0始まり |
| [fenwick.rusk](library/fenwick.rusk) | `Fenwick(n, zero)` / `Fenwick(values, zero)`、`add`、`prefix_sum`、`range_sum` | 可換な加算・減算とゼロ元 |
| [sum_segment_tree.rusk](library/sum_segment_tree.rusk) | `SumSegTree(values, zero)`、`set`、`prod` | 結合的な加算と単位元。集計順は左から右 |

区間は半開区間 `[left, right)` です。添字や区間が不正なら `assert` によるソース位置付きのエラーになります。

検証用の解答は [Unionfind](../library_checker/unionfind.test.rusk)、[Fenwick Treeによる区間和](../library_checker/point_add_range_sum.test.rusk)、[セグメント木による区間和](../library_checker/point_add_range_sum_segment_tree.test.rusk) にあります。公開ケースは `python3 tools/test.py --suite public`、型を変える例・コピー・境界条件は `cargo test --locked --test data_structures --test includes` で確認できます。

構文と共有の説明は [β02の型とライブラリ分割](../../docs/reference/beta02.md) を参照してください。
