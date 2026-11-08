# 関数を使う操作とデータ構造

関数の引数・戻り値は通常どおり共有です。無名関数の型は生成位置ごとに固有で、引数型は明示します。[関数と捕捉](../reference/beta04.md) に文法とコピーの規則をまとめています。

## 列の変換・判定・集計

以下の `Source<T>` は説明上の記号で、配列・整数範囲・対応する遅延イテレータを表します。ソースで書く型名ではありません。

| API | 結果・評価 |
|---|---|
| `map(source, operation)` | `T -> U` の変換を遅延実行するイテレータ。戻り値の `U` は `Unit` 以外でコピー操作に対応 |
| `filter(source, predicate)` | `T -> Bool` が真の要素を共有して通す遅延イテレータ |
| `to_vec(source)` | 残りの要素を消費して `Vec<T>` に共有格納 |
| `all(source, predicate)` | 最初の偽まで消費して偽を返す。空なら真 |
| `any(source, predicate)` | 最初の真まで消費して真を返す。空なら偽 |
| `fold(source, initial, operation)` | 初期値をコピーし、左から `operation(accumulator, element)` の結果へ順に結び直して返す |
| `scan(source, initial, operation)` | 初期値と各集計結果をコピーして保存する配列。N要素の入力にN+1個の結果 |

配列や範囲を渡すと新しい走査を作り、既存の遅延イテレータを渡すとその残りを消費します。`all`・`any` は短絡し、`fold`・`scan` は最後まで走査します。判定結果は `Bool`、集計結果は初期値と同じ型が必要です。

```rusk
let a := [1, 2, 3];
wt a.map((x: Int) => x * 2).to_vec();          // 2 4 6
wt a.fold(10, (sum: Int, x: Int) => sum + x);  // 16
wt a.scan(0, (sum: Int, x: Int) => sum + x);   // 0 1 3 6
wt a.all((x: Int) => x > 0), a.any((x: Int) => x == 2); // Yes Yes
```

`scan` は各段階を再帰的にコピーするので、更新型の操作が同じ蓄積オブジェクトを返しても、それ以前の結果が書き換わりません。`fold` は各戻り値を共有して次へ渡します。コールバックが入力の要素を返したり更新したりする場合は、その共有・更新が観測されます。

時間は消費した要素数と操作・コピーのコストに依存します。遅延変換の走査状態は列全体を保存しません。`to_vec` と `scan` は結果全体の領域を使います。

## 条件付き二分探索

`first_true(left: Int, right: Int, predicate)` は半開区間 `[left, right)` から、条件が初めて真になる位置を返します。全て偽なら `right`、空なら条件を呼ばず `right` を返します。`left > right` は位置付きエラーです。

```rusk
let target := 40;
wt first_true(0, 100, (x: Int) => x * x >= target); // 7
```

条件は `Int -> Bool` で、区間内で偽から真へ単調に変化する必要があります。中点の計算は符号付き64ビットの端をまたぐ区間でもオーバーフローしないようにします。条件の呼び出しはO(log(区間幅+1))回です。条件へ渡した整数を書き換えても探索中の境界を直接変更することはできません。

## キー付き・比較関数付きソート

| API | 条件 |
|---|---|
| `sort_by_key(values, key)` | 各要素 `T` から比較可能なキー `K` を返す |
| `sort_by_comp(values, compare)` | `(T, T) -> Bool`。左を右より前に置くなら真 |

どちらも元の配列を安定ソートし、戻り型は `Unit` です。同値の要素は元の順を保ちます。配列の外側の同一性と、要素の共有を保ちます。並べ替えは走査を無効にする構造変更です。

キー関数は元の順に各要素について一度ずつ実行し、その戻り値を直ちにコピーして保存します。後のキー評価が共有先を更新しても、保存済みのキーは変わりません。空配列では呼びません。キーは基本型・それらの配列・タプルなど、自動的な順序比較に対応する型が対象です。

```rusk
struct Point { x: Int; y: Int; }
let points := [Point { x: 2, y: 0 }, Point { x: 1, y: 9 }];
sort_by_key(points, (p: Point) => (p.x, p.y));
for p in points { wt p.x, p.y; } // 1 9、2 0の順
sort_by_comp(points, (a: Point, b: Point) => a.x > b.x);
```

比較関数は厳密弱順序を満たす必要があります。`compare(a, b)` と `compare(b, a)` が両方偽なら同値です。`<=` をそのまま使うと、同じ値で真になるためこの条件を満たしません。

コールバックから元の配列を構造変更した場合はエラーにします。キー関数は記述した順に捕捉状態を更新でき、要素の中身の更新にも通常の共有規則が適用されます。保存済みのキーはその後の更新から独立します。比較関数は要素や順序に影響する状態を更新しないでください。厳密弱順序や副作用のないことを型で証明する仕組みはありません。ソートはO(N log N)回程度の比較とO(N)の追加領域を使い、キー方式にはN回のキー評価・コピーが加わります。

## 比較関数付き優先度キュー

`examples/beta04/library/priority_queue.rusk` を `include` します。このページのパスはリポジトリ直下のソースから使う想定です。

```rusk
include "examples/beta04/library/priority_queue.rusk";
let queue := PriorityQueue([3, 1, 2], (a: Int, b: Int) => a < b);
queue.push(0);
wt queue.top(), queue.pop(), queue.pop(); // 0 0 1
```

| API | 内容 |
|---|---|
| `PriorityQueue(values: Vec<T>, compare: F)` | 初期配列をコピーしてO(N)でヒープを構築 |
| `push(queue, value)` | 値を再帰的コピーして追加。O(log N) |
| `top(queue) -> T` | 最優先の値の独立したコピー。削除しない |
| `pop(queue) -> T` | 最優先のオブジェクトを削除して共有で返す。O(log N) |
| `len(queue) -> Int` / `is_empty(queue) -> Bool` | 個数・空かどうか。O(1) |

`compare(a, b)` が真なら `a` を優先します。`a < b` で最小、`a > b` で最大の値を先に取り出せます。同順位の取り出し順は保証しません。比較は厳密弱順序が必要で、キューに要素がある間に捕捉状態の変更などで順序を変えないでください。

`top`・`pop` は空なら位置付きエラーです。readonlyがないため、`top` はコピーを返して内部の値の更新を防ぎます。内部フィールドは通常の構造体ですが、直接変更するとヒープの前提が崩れます。`copy(queue)` は値と比較関数の捕捉環境をコピーします。

空から作る場合は `let empty: Vec<Item> := [];` を先に用意し、`PriorityQueue(empty, compare)` として要素型を決められます。

## 演算を指定するセグメント木

`examples/beta04/library/segment_tree.rusk` を `include` します。

```rusk
include "examples/beta04/library/segment_tree.rusk";
let seg := SegTree([1, 2, 3, 4], 0, (a: Int, b: Int) => a + b);
wt seg.prod(1, 4); // 9
seg.set(2, 10);
wt seg.get(2), seg.prod(0, 4); // 10 17
wt seg.max_right(0, (sum: Int) => sum <= 4); // 2
wt seg.min_left(4, (sum: Int) => sum <= 14); // 2
```

| API | 内容 |
|---|---|
| `SegTree(values: Vec<T>, identity: T, operation: F)` | 初期配列からO(N)で構築 |
| `set(seg, position, value)` | 点をコピーで更新。O(log N) |
| `get(seg, position) -> T` | 点の独立したコピー |
| `prod(seg, left, right) -> T` | 半開区間 `[left, right)` を集計。O(log N) |
| `max_right(seg, left, predicate) -> Int` | 左端を固定し、集計が条件を満たす最大の右端 |
| `min_left(seg, right, predicate) -> Int` | 右端を固定し、集計が条件を満たす最小の左端 |

位置は `0 <= position < N`、区間は `0 <= left <= right <= N`。空配列と空区間にも対応し、空区間の結果は単位元です。不正な位置・区間は `assert` で失敗します。

演算は `(T, T) -> T` で、結合的であり、`identity` は左右の単位元である必要があります。引数を更新せず独立した結果を返す規約に従ってください。可換である必要はなく、`prod` は左から右の順序を維持します。文字列連結や一次関数の合成も扱えます。

境界探索の条件は `T -> Bool`、単位元で真、区間を広げると真から偽へ単調に変化することが必要です。単位元で真であることは検査します。`left == N` なら `max_right` はN、`right == 0` なら `min_left` は0を返します。いずれもO(log N)回の集計・判定です。コピーと各要素型の操作のコストは別途加わります。

## 配列の末尾から取り出す

`pop_back(a: Vec<T>) -> T` を追加しています。末尾の格納先を削除して、そのオブジェクトへの共有を返します。既存の共有はそのまま生存します。空配列では位置付きエラー、走査中に使った場合は構造変更として検出します。
