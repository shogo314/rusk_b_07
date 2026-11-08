# β06の実行例

[patterns.rusk](patterns.rusk)は入れ子のenum・タプル・構造体パターン、ガード、or、if let、while let、ラベル付きcontinueを使います。サニタイザーの共通入口でも実行します。

```sh
cargo run --locked -- compile examples/beta06/patterns.rusk -o /tmp/rusk-beta06.cpp
g++ -std=c++20 -O2 /tmp/rusk-beta06.cpp -o /tmp/rusk-beta06
/tmp/rusk-beta06
```

出力は次のとおりです。

```text
6
2 3
1
3
done
```

公開問題では[ITP1_3_C](../aoj/itp1_3_c_patterns.test.rusk)と[ITP1_8_B](../aoj/itp1_8_b.test.rusk)を新構文で記述しています。文法と共有の規則は[パターン分岐](../../docs/reference/beta06.md)を参照してください。

[minmax.rusk](minmax.rusk)は最小・最大演算子と、更新が不要な場合の共有、通常の演算でのコピーを確認します。期待出力は`10 10 6`、`11`、`11 99`です。
