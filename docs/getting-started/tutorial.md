# 小さなプログラムで学ぶ

各例は独立したプログラムです。[導入](index.md)と同じ手順でC++に変換して実行できます。

## 1. 条件と集計

```rusk
in n: Int;
in a(n): Vec<Int>;
let total := 0;
for x in a {
    if x >= 0 { total += x; }
}
wt total;
```

入力は`5`と`3 -2 4 0 -1`、出力は`7`です。`for x in a`は要素を順に取得します。負数を飛ばし、非負の値だけを加算しています。

## 2. 共有とコピーを区別する

```rusk
let a := 10;
let shared := a;
let detached = a;
a = 20;
wt a, shared, detached;
a := 30;
wt a, shared, detached;
```

出力：

```text
20 20 10
30 20 10
```

`=`による宣言はコピー、`:=`による宣言は共有です。宣言後は`=`が中身の更新、`:=`が共有先の変更になります。整数でも同じです。

## 3. 配列の要素も共有する

```rusk
let x := 1;
let a := [x, 2];
a[0] = 8;
wt x, a;
a[0] := 9;
wt x, a;
let independent = a;
independent[0] = 100;
wt a, independent;
```

出力：

```text
8 8 2
8 9 2
9 2 100 2
```

配列リテラルは要素を共有します。`let independent = a;`は内側までコピーします。`wt`は配列を括弧なしで展開するので、`wt x, a;`には整数が3個出ます。

## 4. 関数とif式

```rusk
fn magnitude(x: Int) -> Int {
    return if x < 0 { -x } else { copy(x) };
}
wt magnitude(-7), magnitude(3);
let twice := (x: Int) => {
    let result := x * 2;
    result
};
wt twice(6);
```

出力は`7 3`と`12`の2行です。名前付き関数は戻り型を宣言し、`return`で返します。無名関数は結果の型を推論します。ブロック末尾の式には`;`を付けません。付けるとその値を捨てます。

## 5. 不在をOptionで扱う

```rusk
let a := [4, 7, 9];
if let Some(index) = a.find(7) {
    wt index;
} else {
    wt "missing";
}
wt a.find(100) ?? -1;
```

出力は`1`と`-1`です。`find`は見つかれば`Some(index)`、なければ`None`を返します。`??`は不在のときだけ右辺を評価します。無条件の`?!`は不在時に実行時エラーになるため、不在があり得る場合は分岐や既定値を使います。

## 6. 最小値と最大値を更新する

```rusk
in n: Int;
in a(n): Vec<Int>;
let best := 0;
for x in a { best >?= x; }
wt best;
wt 8 <? 3, 8 >? 3;
```

入力が`4`と`3 1 4 2`なら、出力は`4`と`3 8`です。この例は非負の値の最大値を求める想定で初期値を0にしています。負数だけの配列も扱う場合は、先頭要素やOptionなど、問題に合う初期値を選びます。

次は[制御構文](../reference/control-flow.md)、[関数](../reference/functions.md)、[配列と文字列](../library/sequences.md)で規則とAPIを確認できます。
