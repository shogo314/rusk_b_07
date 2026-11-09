# 共有の持ち出しを制限する

引数名の前に `noescape` を指定すると、その引数や内部の要素・捕捉値への共有を、呼び出し終了後まで外へ残さないことを検査します。省略時は `escape` です。`escape` は持ち出しを許可するだけで、実際の脱出を要求しません。

```text
fn increment(noescape value: Int) { value += 1; }
fn detached(readonly noescape value: Int) -> Int { return copy(value); }
fn shared(escape value: Int) -> Int { return value; }
```

`increment` は引数を更新しますが、共有を外へ残しません。`detached` は独立したコピーを返すため、引数の共有を持ち出しません。`shared` は共有を返す関数なので、`noescape` を指定するとエラーになります。

## 指定できる場所と組み合わせ

名前付き関数、型引数付き関数、無名関数の引数に指定します。`readonly` / `mutable`、`fixed` / `rebindable` と組み合わせられ、順番は自由です。

```text
fn twice(fixed readonly noescape value: Int) -> Int {
    return value * 2;
}
let next := (noescape readonly value: Int) => value + 1;
```

ローカル変数、`let`・`in`・`for`・パターンの束縛、構造体のフィールドには指定できません。`noescape let x := 1;` と `escape let x := 1;` はエラーです。外部関数の引数への指定も、現在はC++本体を検証できないため拒否します。組み込み実装の信頼契約は別の内部情報として登録しています。

重複指定や `noescape escape` の併記はエラーです。注釈だけでオーバーロードを区別することはできません。

## 何を脱出として扱うか

引数自身、引数の要素・フィールド・enumのpayloadへの共有を返す場合を検査します。タプル・構造体・Option・配列の中へ入れて返す場合や、その値を捕捉したクロージャを返す場合も含みます。

```text
// 次の定義はエラー
// fn first(noescape values: Vec<Int>) -> Int { return values[0]; }

fn copied_first(readonly noescape values: Vec<Int>) -> Int {
    return copy(values[0]);
}
```

別名を作っても制約は失われません。仮引数名を別の値へ結び直し、その新しい値を返す場合は、元の引数への共有とは区別します。古い共有を別名に保存して返す場合は依然として脱出です。

別の引数や外部に残る格納先への保存も脱出になります。例えば配列への追加では、配列そのものと追加する値で異なる契約になります。

```text
fn append<T>(noescape values: Vec<T>, escape value: T) {
    values.push_back(value);
}
```

この関数は `values` 自体への共有を外へ持ち出しません。一方、`value` への共有を配列に保持するので、`value` を `noescape` にするとエラーです。

## 呼び出し・返却・コールバック

呼び出し先の注釈だけで判断せず、その本体や既知の組み込み契約から推論した効果を使います。分岐・反復・再帰・相互再帰も解析します。

呼び出し先が共有を返しただけでは、呼び出し元の引数が外へ残るとは限りません。呼び出し元が結果を捨てたり、独立したコピーにしたりする場合と、共有をそのまま返す場合を区別します。

```text
fn alias(value: Int) -> Int { return value; }
fn discard(noescape value: Int) { alias(value); }
fn detached(noescape value: Int) -> Int { return copy(alias(value)); }
```

コールバックも同じです。その場で呼ぶだけでも、結果が捕捉値への共有なら脱出することがあります。

```text
fn invoke<F>(noescape callback: F) -> Int { return callback(); }
fn discard_result<F>(noescape callback: F) { callback(); }
let value := 3;
wt invoke(() => value + 1);     // 新しい結果なので許可
// wt invoke(() => value);     // 捕捉値への共有を返すためエラー
discard_result(() => value);  // 結果を捨てるので許可
```

型引数付きの関数は、実際の型・コールバックで具体化した本体を検査します。未使用の型引数付き関数の全組み合わせを検証する機能ではありません。

## 現在の保守的な検査

β07の契約検査は保守的です。未登録の外部実装やコンテナ操作は安全と仮定しません。例えば配列の走査イテレータ、遅延map/filterなどには未解析の操作があり、実際には呼び出し内で使うだけでも拒否する場合があります。

直接構築した構造体・タプルなどの格納先について、すべての別名が関数内に留まると証明できれば、フィールドへの後からの格納も許可します。

```text
struct Box { value: Int; }
fn local(noescape value: Int) -> Int {
    let box := Box { value: 0 };
    let alias := box;
    alias.value := value;
    return box.value + 1;
}
```

別名やローカル変数の結び直しも含めて関数全体を確認します。格納先を別の私的な構造体・タプル・enumの中に入れ、フィールドやpayloadから取り出す場合も、到達する共有先を追跡します。

```text
fn nested(noescape value: Int) -> Int {
    let child := Box { value: 0 };
    let parent := (child, 0);
    parent[0].value := value;
    return parent[0].value + 1;
}
```

親を返す・関数へ渡す・外部の格納先へ保存する場合は、その親から到達する子も公開済みとして扱います。格納前の公開、分岐の別の経路、ループで過去に格納した子も対象です。一方、子の一つだけを関数へ渡しても、親や兄弟まで自動的に公開済みにはしません。

既知の関数が引数自身やそのフィールド・payloadを共有して返す場合も、解析した関係を実引数へ対応させます。呼び出し先がその引数を更新・構造変更・外部保持せず、外部状態も更新しないと証明できれば、受け渡しだけで私的な共有先を公開済みとは扱いません。

```text
fn identity(box: Box) -> Box { return box; }
fn via_call(noescape value: Int) -> Int {
    let box := Box { value: 0 };
    let alias := identity(box);
    alias.value := value;
    return box.value + 1;
}
```

構造体・タプル・enum・Optionなどの内部に包んで返される共有も、返却結果から元の共有先への到達関係として追跡します。関数内に留めるだけなら、元の別名を使った私的な格納を妨げません。

```text
fn pack(box: Box) -> (Box, Int) { return (box, 0); }
fn via_container(noescape value: Int) -> Int {
    let box := Box { value: 0 };
    let held := pack(box);
    box.value := value;
    return held[0].value + 1;
}
```

`held`やその内部の共有を外へ渡す場合は、元の`box`も公開済みとして扱います。格納前の公開や、分岐・ループをまたぐ公開も対象です。

返却情報は共有元の候補を表しており、新しい値を返すことだけで一意な所有は証明しません。上の例で`box.value := value;`を`held[0].value := value;`へ変更した場合は、取得先が必ず元の`box`だけであるとは証明せず、保守的に拒否します。返却された集約自身を格納先とする場合にも制限があります。

更新・保持があり得る引数は公開済みとし、その先のフィールドも保守的に扱います。未知の呼び出しや返却関係、外部書き込みがある場合も同様です。配列操作や、返却された集約の格納先の証明を一般化した解析は今後の作業です。

格納後のフィールドから取り出した値の共有元には、引き続き不明な扱いが残ります。例えば上の関数を `return box.value;` に変えると、noescapeを証明できず拒否します。独立した演算結果やコピーの返却とは区別します。

Optionの構築・判定・値取得には既知の契約を登録しています。`Some(value)` は新しいOptionを作って内部へ共有を保持し、`value` / `?!` はpayloadへの共有を返します。これらを呼んだだけで外部へ保存されたとは判定せず、その後の利用を追跡します。Option全体のコピーなど、ほかの操作には未解析のものも残っています。

これらの解析精度は今後改善します。注釈がなくても同じ効果解析を行い、`escape` を明示した場合も、証明できた非脱出性をIRへ記録します。[引数借用と閉じたBoolの値保持](../development/beta07-performance.md)にも解析結果を利用します。注釈そのものを最適化の根拠にはしません。

## 診断とIR

共有の返却・保持を検出すると `noescape parameter 'value' may escape` を表示します。解析が不明、または格納先の寿命などを証明できない場合は `cannot prove noescape for parameter 'value'` と区別します。

違反する `return`・保存・呼び出しの位置、引数と `noescape` の宣言位置を示します。呼び出しでは関数名と引数番号、共有元では要素・フィールド・捕捉などの射影経路を示します。さらに`sharing path`で、同じ関数内の別名の宣言、名前の結び直し、フィールドやpayloadへの格納・取得、呼び出し結果への共有を順に示します。別名は元のソース名と位置を表示し、シャドーイングや結び直しの前に捨てた共有を、現在の違反経路として表示しません。呼び出し結果では、引数番号に加えて、取得元と返却結果の内部の経路も示します。

共有元を証明できない場合は`uncertain sharing path`とし、ヒープ情報を失った格納・呼び出しや、返却関係が不明な関数を示します。解析上限が原因になり得る場合も不明性を説明し、関係を証明できない引数からの別名経路を作りません。診断用の履歴は16段までとし、長い場合は省略を明示します。経路表示の省略で契約検査の結果は変わりません。

通常の関数や静的コールバックでは、対応する仮引数から局所別名・共有の返却まで、呼び出し先の本体内の経路も示します。呼び出し先が値を外へ保持する場合は、受け渡された引数に対応する別名と格納・再呼び出しの位置を示します。別の引数やフィールドの経路を代わりに表示することはしません。

本体のない外部実装、未知の共有元、再帰、診断用の解析上限では、内部経路を復元できない理由を表示します。呼び出し先の追跡は16段、表示は呼び出し元と合わせて16段まで、命令・状態の複製・合流などの追加解析は100万単位までです。これらは静的解析上の共有候補であり、実行済みの履歴ではありません。格納先が私的と証明できなかった理由を、公開先まで完全に列挙する処理には未対応です。readonlyの詳細経路は[読み取り専用の参照](access.md)で説明しています。

ASTは宣言した注釈、HIR/LIRは `AnnotateEscape` と、関数の `effects` にある `escape` / `retained` を保持します。`escape` は返却を含む持ち出し、`retained` は戻り値以外への保持です。IRには `return` の位置も保持します。`DescribeBinding`は注釈のない変数も含めて名前・位置を残す診断用の情報です。捕捉した変数では最初の取得位置と捕捉であることを残します。これらの情報は効果や所有権の証明には使いません。宣言を根拠に推論結果を上書きせず、検査を通してからHIR/LIR・C++を出力します。注釈は実行時の処理やC++の引数の型を変更しません。
