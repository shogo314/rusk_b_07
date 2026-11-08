# 制御構文

## if文とif式

`if condition { ... }`は文として使え、`else`を省略できます。値が必要なif式では`else`が必要です。条件に丸括弧は不要で、本体の波括弧は必要です。`else if`は未対応なので、`else { if ... }`と書きます。

```rusk
let score := 72;
let passed := if score >= 60 { "pass" } else { "fail" };
wt passed;
```

出力は`pass`です。後続へ到達する分岐の結果型は一致させます。真偽値以外も条件にでき、整数・文字のゼロ、空文字列・空配列は偽です。対応型の詳細は[型・共有・評価](semantics.md)を参照してください。

## ブロックの値とスコープ

```rusk
let answer := {
    let base := 6;
    base * 7
};
wt answer;
```

出力は`42`です。末尾のセミコロンなしの式が結果で、末尾式がなければ`Unit`になります。内側で宣言した名前は外へ持ち出せませんが、結果として返したオブジェクトは生存します。`{}`もUnitの値を作れます。

ブロックはジャンプ先のラベルを持ちません。ブロックの中から`break`・`continue`で囲んでいるループへ移動することはできます。値付き`break`はありません。[ブロック式とラベル](blocks-labels.md)に詳しい例があります。

## forとwhile

```rusk
let a := [2, 4, 6];
for i in 0..a.len() { wt i, a[i]; }
```

出力は`0 2`、`1 4`、`2 6`の3行です。`start..end`は終端を含まない昇順の整数範囲です。開始値が終端以上なら空です。端点は走査開始時に一度だけ評価します。

`for x in a`は要素を共有します。`x = value`は元の要素を更新しますが、`x := value`はループ変数の共有先だけを変えます。配列の構造を走査中に変更すると、次の反復検査でエラーになります。

`while condition { ... }`は毎回条件を評価し、`loop { ... }`は`break`などで抜けるまで反復します。`break;`と`continue;`は最も内側のループに作用します。

## 自然終了とelse、ラベル

```rusk
let values := [1, 3, 5];
for x in values {
    if x % 2 == 0 { wt x; break; }
} else {
    wt "no even value";
}
```

出力は`no even value`です。`for`・`while`の`else`は、そのループを`break`せず自然終了した場合に実行します。最初から空の走査・偽の条件も含みます。`loop`に`else`はありません。

`outer: for ...`のようにループへ名前を付け、内側から`break outer;`・`continue outer;`で対象を指定できます。関数の境界を越えるラベル参照はできません。

## パターンによる条件と分岐

`if let pattern = value { ... } else { ... }`は一致した場合だけ束縛した名前を本体で使えます。式にもできます。`while let pattern = value { ... }`は毎回右辺を一度評価します。複数の場合分けには`match`文・式を使います。

[パターン分岐・ガード・if let](beta06.md)には、入れ子、orパターン、ガードの評価順、網羅性、変数のスコープをまとめています。`match`では文として使う場合にも網羅性を検査します。
