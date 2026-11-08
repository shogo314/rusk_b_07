# 名前の結び直しを制限する

`fixed` は、初期化した名前を `:=` で別の値へ結び直すことを禁止します。省略時は `rebindable` です。明示しても省略時と同じ動作になります。

```text
fixed let count := 1;
count = 2;                 // 共有している値の更新は可能
count += 3;
// count := 9;             // エラー：初期化後の結び直し
rebindable let current := count;
current := 9;
wt count, current;         // 5 9
```

## 指定できる場所

`let` の前、名前付き関数・型引数付き関数・無名関数の引数名の前に指定します。分解宣言では各変数に適用します。

```text
fixed let (left, right) := (1, 2);
fn increment(fixed value: Int) -> Int {
    value += 1;
    return value;
}
let next := (fixed value: Int) => value + 1;
```

引数の指定は、その関数内の仮引数名に対する制限です。呼び出し元の名前へ制限を付け加えません。注釈だけでオーバーロードを区別することもできません。

`fixed fixed` のような重複や `fixed rebindable` のような相反する指定はエラーです。外部関数の引数、構造体のフィールド、`in`・`for`・パターンの束縛には指定できません。[readonly / mutable](access.md) との組み合わせにも対応します。引数には[noescape / escape](escape.md)も指定できます。

## 中身の更新と別名

`fixed` は名前の制限です。要素・フィールドの更新や結び直し、配列への追加は禁止しません。

```text
fixed let values := [1, 2];
values[0] := 3;
values.push_back(4);
let alias := values;
alias := [9];              // 新しい名前にはfixedを引き継がない
```

同名の新しい宣言にも自動では引き継ぎません。一方、無名関数が外側の固定変数を捕捉した場合は同じ名前への制限を維持し、その捕捉名の結び直しを拒否します。中身の更新は捕捉先からも可能です。

## 初期化と制御フロー

未初期化で宣言し、後で一度だけ初期化することもできます。

```text
fn choose(flag: Bool) -> Int {
    fixed let answer: Int;
    if flag { answer := 1; } else { answer := 2; }
    return answer;
}
```

いずれかの経路ですでに初期化されている可能性があれば、その後の `:=` を拒否します。ループの反復で同じ名前を二度初期化する場合もエラーです。ループ本体で毎回新しく宣言した名前は、反復ごとに初期化できます。`break`・`continue`・ラベル付きの移動も検査します。

条件の値を一般に証明する検査ではなく、制御フロー上の候補を保守的に扱います。既存の初期化前使用検査にも従うため、ループ内だけで初期化した変数をループの後で読むことは、実行時に必ず初期化される例でも拒否する場合があります。

## 診断とIR

違反では結び直しの位置と、変数・`fixed` を宣言した位置を表示します。注釈はAST、HIR、LIRから確認でき、HIR/LIRでは `AnnotateBinding` として保持します。C++生成前に制御フローを検査します。この注釈自体は実行時の処理やC++の `const` を追加しません。
