# ブロック式とラベル付きループ

β05では、ブロック内で文を実行し、最後の式を値として返せます。if式の各分岐にも同じブロックを使います。ループにラベルを付け、内側のブロック式やループから対象を指定して脱出・継続できます。

## ブロックの結果とスコープ

```text
let result := {
    let x := 10;
    let y := x + 2;
    x * y
};
wt result; // 120
```

末尾の式にはセミコロンを付けません。末尾の式がない場合や、最後の式にセミコロンを付けた場合は `Unit` です。空の `{}` もUnitを返します。

```text
fn consume(value: Unit) -> Int { return 7; }
let empty: Unit := {};
let discarded: Unit := { 1 + 2; };
wt consume(empty), consume(discarded);
```

内側で宣言した名前は外へ見えません。外側の名前は参照でき、内側で同じ名前を宣言すると隠せます。結果はコピーせず共有として返します。独立させたい場合はコピー宣言を使います。

```text
let source := [1];
let held := { let local := source; local };
held[0] = 7;                // sourceも7
let independent = { source };
independent[0] = 9;          // sourceは7のまま
wt source, independent;
```

戻すオブジェクトや文字への共有は、ブロックを抜けても生存します。

```text
let letter := { let text := "abc"; text[1] };
wt letter; // b。元の文字列も共有が残る間は生存する
```

ブロック式は引数、代入、配列要素、条件などの式の位置で使えます。文頭で `{ ... }` と書いた場合はブロック文として扱い、式の結果を捨てます。文頭のブロックを演算・呼び出しへつなぐ場合は `({ ... })` と括弧で囲みます。ブロック文の後の `;` は省略できます。

## 複数文を持つif式

```text
let flag := true;
let value := if flag {
    let base := 10;
    base + 1
} else {
    let base := 20;
    base * 2
};
wt value; // 11
```

条件と選択した分岐だけを評価します。値を返して継続する分岐の型は一致する必要があります。if式にはelseが必要です。

ブロックの末尾にif/elseを書くと、その結果をブロックの結果にできます。

```text
let value := {
    let flag := false;
    if flag { 10 } else { 20 }
};
wt value; // 20
```

末尾のif/elseに `;` を付けると結果を捨て、外側のブロックはUnitになります。通常のif文では分岐の結果を使いません。

## ラベル付きのbreakとcontinue

```text
let found := false;
search: for i in 0..3 {
    for j in 0..3 {
        if i == 1 && j == 2 {
            found = true;
            break search;
        }
    }
}
wt found; // Yes
```

`名前: for`、`名前: while`、`名前: loop` と書きます。

| 書き方 | 動作 |
|---|---|
| `break;` | 最も内側のループを抜ける |
| `continue;` | 最も内側のループの次の反復へ進む |
| `break label;` | 指定したループを抜ける |
| `continue label;` | 指定したループの次の反復へ進む |

```text
let total := 0;
rows: for i in 0..3 {
    for j in 0..3 {
        if j == 1 { continue rows; }
        total += i;
    }
}
wt total; // 3
```

ラベルは変数・関数とは別の名前です。変数と同じ綴りを使えます。同じラベルを別々の終了済みのループで再利用できますが、有効な外側のラベルと同じ名前を内側のループへ付けることはできません。

有効範囲は `for` の本体、`while` の条件と本体、`loop` の本体です。`for`の走査元の式では、そのfor自身のラベルはまだ有効ではなく、既に有効な外側のループだけを指定できます。ループのelseでは、そのループ自身のラベルは有効ではありません。関数やクロージャの境界を越えてラベルを指定することもできません。

ラベルとコロン、コロンとループの間には空白・改行・行コメントを置けます。ラベル名だけを後から宣言する構文や、前方のループへジャンプする構文はありません。

## ブロック式から外側へ抜ける

```text
outer: for i in 0..5 {
    let doubled := {
        if i == 3 { break outer; }
        i * 2
    };
    wt doubled;
}
// 0、2、4をそれぞれ一行で出力
```

脱出した経路では、`doubled`の初期化も後続の出力も実行しません。抜けるスコープに属する共有は解放し、外側に残る共有は保ちます。

値を返す分岐と脱出する分岐を混在できます。

```text
for i in 0..5 {
    let value := if i == 3 { break; } else { i * 2 };
    wt value;
}
```

`return`も同様です。値を返して続く経路だけを使って型と確実な初期化を検査します。

```text
fn calculate(n: Int) -> Int {
    let value := if n < 0 {
        return 0;
    } else {
        let next := n + 1;
        next
    };
    return value * 2;
}
wt calculate(-1), calculate(3); // 0 8
```

`??`の右辺や、比較の連鎖・論理演算の後続部分にもブロックを置けます。未選択・短絡した側の脱出は実行しません。

```text
outer: loop {
    let value := Some(42) ?? { break outer; };
    wt value; // 42
    break;
}
```

## クロージャのブロック本体

```text
let offset := 10;
let calculate := (n: Int) => {
    if n < 0 { return offset; }
    let next := n + 1;
    next * 2
};
wt calculate(-1), calculate(3); // 10 8
```

末尾の値と明示的なreturnから戻り型を決め、継続・返却する経路の型を照合します。returnの対象はそのクロージャです。内側の宣言は捕捉せず、自由変数だけを捕捉します。捕捉環境の共有・コピーは[関数と捕捉](beta04.md)の規則に従います。

名前付き関数の本体は引き続き文のブロックです。非Unit関数では明示的に `return` する必要があり、関数本体の末尾に `x + 1` と書くだけでは戻り値になりません。`return { let x := 1; x + 1 };` は使えます。

## ループのelseと制限

対象ループをbreakで抜けると、そのelseは実行しません。外側へbreak/continueした場合、途中で抜けた内側のループのelseも実行しません。外側へのcontinueの後、その外側のループが正常に走査を終えた場合は、そのelseを実行します。

通常のwhile/for本体で行った初期化を、終了後に必ず実行済みとする解析は引き続き保守的です。break/continueによる脱出経路は分岐の型から除外しますが、一般の到達可能性や条件式の値は証明しません。脱出に続く文は検査用の文脈で名前・型を確認し、実行コードからは除きます。値を返さない式の後続オペランドを完全に型推論する、一般のNever型の仕組みは導入していません。

ラベルを付けられるのはループです。ラベル付きの通常ブロック、`break label value`による値の返却、値を返すループ、一般のNever型は未対応です。`return`の対象にラベルを付ける機能もありません。

## 検証

通常版231テスト・最適化無効版117テスト、ASan/UBSan/LeakSanitizerの13例、公開49解答805ケースを検証しています。このページのプログラム例13件もC++へ変換して実行しています。

変更前との10項目の性能比較では、7回の中央値の差は約−4〜＋5%でした。既存の入出力・一時値の確保削減を維持しています。測定条件と各回の値は[測定JSON](../development/blocks-labels-benchmark.json)に記録しています。単一環境の短時間の比較で、数%の差を有意な改善・悪化とは扱いません。
