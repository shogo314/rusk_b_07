# Rusk β07

Ruskは、競技プログラミング向けに開発している言語です。RuskのソースをC++20へ変換し、生成したC++をコンパイルして実行・提出します。このサイトは**β07開発版の実装済みの言語と標準ライブラリ**を説明します。開始時点ではβ06の機能を引き継いでいます。関数の効果と戻り値の共有元を求める初期解析を実装中です。fixed/rebindableとreadonly/mutableの構文・初期の契約検査は実装済みです。noescape/escapeと、新しい借用・スカラー最適化は未実装です。[β07の開始点と方針](development/beta07.md)に計画をまとめています。将来の設計案とは区別しています。

```rusk
in n: Int;
in a(n): Vec<Int>;
let best := 0;
for x in a { best >?= x; }
wt a.sum(), best;
```

入力が `4` と `3 1 4 2` なら、出力は `10 4` です。トップレベルに実行文を書き、`main`関数は定義しません。

## 最初に読むページ

1. [導入と実行](getting-started/index.md)：必要な環境、ビルド、C++への変換。
2. [小さなプログラムで学ぶ](getting-started/tutorial.md)：入力、配列、共有、関数、Option。
3. [基本構文と共有](reference/index.md)：`=`と`:=`、評価順、添字、スコープ。
4. [標準ライブラリ](library/index.md)：操作を目的から探す。

## 書き方を調べる

| 調べたいこと | ページ |
|---|---|
| 構文・演算子の優先順位 | [文法一覧](reference/grammar.md) |
| コピーと共有、代入、型検査 | [型・共有・評価](reference/semantics.md) |
| if式、ループ、ラベル、ブロック | [制御構文](reference/control-flow.md) |
| 関数、型引数、クロージャ | [関数](reference/functions.md) |
| 構造体、タプル、ファイル分割 | [型とinclude](reference/beta02.md) |
| enum、Option、パターン分岐 | [enum](reference/enums.md)、[パターンとガード](reference/beta06.md) |
| 最小・最大値の選択と更新 | [最小・最大演算子](reference/minmax.md) |
| 入力形式、EOF、出力 | [入出力](library/input-output.md) |
| 配列・文字列のAPIと共有 | [配列と文字列](library/sequences.md) |
| 対応範囲と制限 | [対応機能](reference/status.md)、[エラー](reference/errors.md) |

## Ruskの共有

`let b := a;` は同じオブジェクトを共有し、`let b = a;` はコピーします。既存変数に対する `a = value` は共有中の中身を更新し、`a := value` はその変数の共有先を変更します。基本型の整数にもこの規則が適用されます。

## 実装と検証

[変換パイプライン](development/pipeline.md)では、CST・AST・型付けしたHIR・制御フローを表すLIRからC++生成までを説明します。[テストの実行](development/verification.md)ではローカルとCIの確認方法を扱います。版ごとの変更・性能測定は開発ガイドにまとめています。

このサイトのコードブロックには、単独で実行するプログラムと、説明用の構文断片があります。入力・出力を添えた例は、それらの値で動作を確認できるようにしています。
