# 導入と実行

Rusk β07はソースをC++20へ変換する処理系です。Rustのstableツールチェーンと、C++20対応のGCCまたはClangを用意してください。以下はLinuxで`g++`を使う例です。生成コードは`__int128`も使うため、すべてのC++20コンパイラでの動作を保証するものではありません。

## リポジトリを用意する

```bash
git clone https://github.com/shogo314/rusk_b_07.git
cd rusk_b_07
cargo build --release --locked
```

すでにソースがある場合は、そのリポジトリのルートでビルドします。生成される処理系は`target/release/rusk`です。

## 最初のプログラム

`main.rusk`を次の内容で作ります。

```rusk
in a, b: Int;
wt a + b;
```

C++への変換、コンパイル、実行を順に行います。

```bash
./target/release/rusk compile main.rusk -o /tmp/main.cpp
g++ -std=c++20 -O2 /tmp/main.cpp -o /tmp/main
printf '10 32
' | /tmp/main
```

出力：

```text
42
```

オンラインジャッジには生成した`/tmp/main.cpp`を提出します。実行時にRuskの処理系は必要ありません。

## 配列を入力する

```rusk
in n: Int;
in a(n): Vec<Int>;
wt a.sum();
```

入力：

```text
4
3 1 4 2
```

出力：

```text
10
```

`in`は変数の宣言を兼ねます。`a(n)`の`n`は配列の長さで、入力から長さを自動推定する記法ではありません。詳しくは[入出力](../library/input-output.md)を参照してください。

## 次に読む

[チュートリアル](tutorial.md)で共有と分岐を学べます。[コマンドライン](cli.md)には生成先や中間表現の出力方法、[対応機能](../reference/status.md)には実装範囲をまとめています。

動くプログラムはリポジトリの`examples/`にもあります。`examples/prototype/`は配列・探索・BFS・再帰、`examples/beta06/`はパターン分岐と最小・最大演算子の例です。[テスト](../development/verification.md)を実行する場合にはPythonなどの追加環境を使います。
