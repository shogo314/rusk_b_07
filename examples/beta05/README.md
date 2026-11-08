# β05の実行例

`features.rusk` は辞書・集合、Optionの共有と不在、if式、比較の連鎖、配列findを入力なしで試す例です。

```bash
cargo run --locked -- compile examples/beta05/features.rusk -o /tmp/rusk-beta05.cpp
g++ -std=c++20 -O2 /tmp/rusk-beta05.cpp -o /tmp/rusk-beta05
/tmp/rusk-beta05
```

出力:

```text
2 0
1 Yes
9 2
Yes 9 2
Yes
small
0
```

公開問題を使う例:

- [Associative Array](../library_checker/associative_array.test.rusk)：Dictの登録と非挿入のget、??。
- [ALDS1_4_C](../aoj/alds1_4_c.test.rusk)：StrのSet、if式。
- [ITP1_6_Dの別解](../aoj/itp1_6_d_bulk.test.rusk)：多次元配列の一括入力。
- [ITP1_3_Cの別解](../aoj/itp1_3_c_option.test.rusk)：try_read、?!、タプルを返すif式。

`python3 tools/test.py` で既存の例と一緒に検証します。

`control_flow.rusk` は入れ子のループのbreak/continue、関数から返す文字とローカルの文字列の寿命を確認する例です。`--emit cst|ast|hir|lir|cpp` で[処理段階](../../docs/development/pipeline.md)を確認できます。

`blocks_and_labels.rusk` はブロックから返す文字、ラベル付きbreak/continueでの解放、ブロック内の変数を捕捉するクロージャの例です。出力は `a b`、`Zbc!`、`1 2` の3行です。
