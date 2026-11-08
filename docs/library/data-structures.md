# Ruskで記述したデータ構造

`examples/beta02/library/` はRuskで書いた再利用可能なライブラリです。標準で自動読み込みされるものではないため、利用するファイルを `include` します。このページの例はリポジトリ直下の `main.rusk` に保存する想定です。別の場所へ置く場合は、そのファイルからの相対パスに直してください。

構造体のフィールドは公開されていますが、通常は以下の関数を通して操作します。内部の配列や長さを直接変更するとデータ構造の前提が崩れます。関数への受け渡しは共有なので、更新は呼び出し元の状態にも反映されます。独立した状態が欲しい場合は `let other = value;` を使います。

## Union-Find

```rusk
include "examples/beta02/library/union_find.rusk";
let uf := UnionFind(4);
uf.unite(0, 1);
uf.unite(1, 2);
wt uf.same(0, 2), uf.same(0, 3); // Yes No
let independent = uf;
independent.unite(2, 3);
wt uf.same(0, 3), independent.same(0, 3); // No Yes
```

| 関数 | 内容 |
|---|---|
| `UnionFind(n: Int) -> UnionFind` | 頂点 `0..n` の集合を作る。`n >= 0` |
| `leader(uf: UnionFind, vertex: Int) -> Int` | 代表頂点を返す。経路圧縮で状態を更新する |
| `unite(uf: UnionFind, u: Int, v: Int)` | 2集合を併合。既に同じなら何もしない。戻り型は `Unit` |
| `same(uf: UnionFind, u: Int, v: Int) -> Bool` | 同じ集合かを返す。代表探索で状態を更新する |

頂点は `0 <= vertex < n`。構築はO(n)、サイズによる併合と経路圧縮により一連の操作は償却O(α(n))です。コピーはO(n)です。存在しない頂点へのアクセスは配列の範囲検査で失敗します。

## Fenwick Tree

```rusk
include "examples/beta02/library/fenwick.rusk";
let fw := Fenwick([1, 2, 3, 4], 0);
wt fw.prefix_sum(3), fw.range_sum(1, 4); // 6 9
fw.add(1, 10);
wt fw.range_sum(1, 3); // 15
let empty := Fenwick(0, 0);
wt empty.range_sum(0, 0); // 0
```

| 関数 | 内容 |
|---|---|
| `Fenwick<T>(n: Int, zero: T) -> Fenwick<T>` | `n` 要素をゼロ元で初期化。`n >= 0` |
| `Fenwick<T>(values: Vec<T>, zero: T) -> Fenwick<T>` | 初期配列から構築 |
| `add<T>(fw: Fenwick<T>, position: Int, delta: T)` | 位置に `delta` を加算 |
| `prefix_sum<T>(fw: Fenwick<T>, end: Int) -> T` | 半開区間 `[0, end)` の集計 |
| `range_sum<T>(fw: Fenwick<T>, left: Int, right: Int) -> T` | 半開区間 `[left, right)` の集計 |

更新位置は `0 <= position < n`、区間は `0 <= left <= right <= n`。空区間はゼロ元です。不正な位置・区間は `assert` で失敗します。初期配列と内部状態は独立するため、構築後に元の配列を書き換えても木は変わりません。

`T` は可換な加算と減算、コピーを提供し、`zero` はそのゼロ元である必要があります。加減算は引数を変更せず独立した結果を返すように定義します。処理系は可換性などの数学的性質を証明しません。

構築はO(n)、更新・区間集計はO(log n)、コピーはO(n)。要素型の演算・コピーを定数時間とした計算量です。整数の加算・減算には通常どおりオーバーフロー検査が適用されます。

## 区間和セグメント木

```rusk
include "examples/beta02/library/sum_segment_tree.rusk";
let seg := SumSegTree([1, 2, 3, 4], 0);
wt seg.prod(1, 4); // 9
seg.set(2, 10);
wt seg.prod(0, 4); // 17
let words := SumSegTree(["ab", "cd", "ef"], "");
wt words.prod(0, 3); // abcdef
words.set(1, "X");
wt words.prod(0, 3); // abXef
```

| 関数 | 内容 |
|---|---|
| `SumSegTree<T>(values: Vec<T>, zero: T) -> SumSegTree<T>` | 配列から構築 |
| `set<T>(seg: SumSegTree<T>, position: Int, value: T)` | 要素を `value` のコピーで置換 |
| `prod<T>(seg: SumSegTree<T>, left: Int, right: Int) -> T` | 半開区間 `[left, right)` を左から右へ集計 |

更新位置は `0 <= position < n`、区間は `0 <= left <= right <= n`。空配列の構築と空区間の集計も可能です。空区間は単位元を返します。不正な位置・区間は `assert` で失敗します。

`T` は結合的な `+` とコピーを提供し、`zero` はその単位元です。`+` は引数を変更せず独立した結果を返すように定義します。可換である必要はなく、文字列連結のように順序を持つ演算にも使えます。任意の演算関数を引数に取る汎用セグメント木ではないため、最小値などを扱いたい場合は演算を包む型や別の実装を用意します。

構築はO(n)、点更新・区間集計はO(log n)、コピーと保存領域はO(n)。文字列の連結などは要素の長さに応じた演算・コピーのコストが加わります。

## 自作の型に演算を与える

通常のRusk関数として演算フックを定義できます。次は整数の重みを包む構造体です。構造体の `copy` は自動生成されます。

```rusk
include "examples/beta02/library/fenwick.rusk";
struct Weight { value: Int; }
fn Weight(value: Int) -> Weight { return Weight { value: value }; }
fn __add__(a: Weight, b: Weight) -> Weight { return Weight(a.value + b.value); }
fn __sub__(a: Weight, b: Weight) -> Weight { return Weight(a.value - b.value); }
let fw := Fenwick([Weight(2), Weight(5)], Weight(0));
fw.add(0, Weight(3));
wt fw.range_sum(0, 2).value; // 10
```

構造体全体の出力は自動生成しないため、例では結果の `.value` を出力します。利用に必要な演算は型を具体化したときに検査します。演算フックの名前は [演算と共有](../reference/index.md) と標準ライブラリの実装も参照してください。
