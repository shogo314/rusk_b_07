# AtCoder の解答例

Ruskの実装済み機能で解いた100問です。公式問題ページを確認し、各問題の制約に対応する解法を書いています。AtCoderへの提出・AC確認は行っていません。

問題文そのものは収録せず、リンクと自作の解答、公式の入出力サンプル（`samples.json`）を置いています。テストは各解答をC++20に変換してコンパイルし、すべての公式サンプル279件で出力トークンを照合します。サンプル一致は全入力での正しさや制限時間内の実行を保証しません。

全テストの共通入口は `python3 tools/test.py`、E2EとAtCoderサンプルを含む手元の確認は `python3 tools/test.py --suite local` です。AtCoderだけ・問題単位の実行は次のコマンドで行えます。

```bash
cargo test --locked --test atcoder
cargo test --locked --test atcoder abc240_c
cargo run --locked -- compile examples/atcoder/abc240_c.rusk -o /tmp/abc240_c.cpp
g++ -std=c++20 -O2 /tmp/abc240_c.cpp -o /tmp/abc240_c
```

選定では、今の言語で素直に書ける問題を優先しています。浮動小数点や専用データ構造など、未実装機能を補うために解答が複雑になる問題は見送ります。

入力値から独立した作業用変数は `let x = source;` で宣言できます。共有する場合は `let x := source;` を使います。可変の変数を配列へ保存するときは `push_back(copy(x))` のように必要に応じてコピーを使います。`abc081_b`、`abc289_b`、`abc320_b` などが例です。

| 問題 | 題名 | 解法 | 解答 | サンプル数 |
|---|---|---|---|---|
| [ABC050 B](https://atcoder.jp/contests/abc050/tasks/abc050_b) | B - Contest with Drinks Easy | 合計と差分更新 | [abc050_b.rusk](abc050_b.rusk) | 2 |
| [ABC079 B](https://atcoder.jp/contests/abc079/tasks/abc079_b) | B - Lucas Number | 漸化式 | [abc079_b.rusk](abc079_b.rusk) | 2 |
| [ABC080 B](https://atcoder.jp/contests/abc080/tasks/abc080_b) | B - Harshad Number | 桁の和 | [abc080_b.rusk](abc080_b.rusk) | 3 |
| [ABC081 A](https://atcoder.jp/contests/abc081/tasks/abc081_a) | A - Placing Marbles | 文字の計数 | [abc081_a.rusk](abc081_a.rusk) | 2 |
| [ABC081 B](https://atcoder.jp/contests/abc081/tasks/abc081_b) | B - Shift only | 整数の反復除算 | [abc081_b.rusk](abc081_b.rusk) | 3 |
| [ABC083 B](https://atcoder.jp/contests/abc083/tasks/abc083_b) | B - Some Sums | 関数と桁の和 | [abc083_b.rusk](abc083_b.rusk) | 3 |
| [ABC084 B](https://atcoder.jp/contests/abc084/tasks/abc084_b) | B - Postal Code | 文字の検査 | [abc084_b.rusk](abc084_b.rusk) | 3 |
| [ABC085 A](https://atcoder.jp/contests/abc085/tasks/abc085_a) | A - Already 2018 | 文字の更新 | [abc085_a.rusk](abc085_a.rusk) | 2 |
| [ABC085 B](https://atcoder.jp/contests/abc085/tasks/abc085_b) | B - Kagami Mochi | ソートと重複除去 | [abc085_b.rusk](abc085_b.rusk) | 3 |
| [ABC086 A](https://atcoder.jp/contests/abc086/tasks/abc086_a) | A - Product | 偶奇の判定 | [abc086_a.rusk](abc086_a.rusk) | 2 |
| [ABC086 B](https://atcoder.jp/contests/abc086/tasks/abc086_b) | B - 1 21 | 整数の平方 | [abc086_b.rusk](abc086_b.rusk) | 3 |
| [ABC087 A](https://atcoder.jp/contests/abc087/tasks/abc087_a) | A - Buying Sweets | 剰余 | [abc087_a.rusk](abc087_a.rusk) | 4 |
| [ABC087 B](https://atcoder.jp/contests/abc087/tasks/abc087_b) | B - Coins | 三重ループの全探索 | [abc087_b.rusk](abc087_b.rusk) | 3 |
| [ABC088 A](https://atcoder.jp/contests/abc088/tasks/abc088_a) | A - Infinite Coins | 剰余と比較 | [abc088_a.rusk](abc088_a.rusk) | 3 |
| [ABC088 B](https://atcoder.jp/contests/abc088/tasks/abc088_b) | B - Card Game for Two | ソートと貪欲法 | [abc088_b.rusk](abc088_b.rusk) | 3 |
| [ABC089 B](https://atcoder.jp/contests/abc089/tasks/abc089_b) | B - Hina Arare | 配列の検索 | [abc089_b.rusk](abc089_b.rusk) | 3 |
| [ABC090 B](https://atcoder.jp/contests/abc090/tasks/abc090_b) | B - Palindromic Numbers | 桁の比較 | [abc090_b.rusk](abc090_b.rusk) | 2 |
| [ABC092 B](https://atcoder.jp/contests/abc092/tasks/abc092_b) | B - Chocolate | 整数の切り上げ | [abc092_b.rusk](abc092_b.rusk) | 3 |
| [ABC094 B](https://atcoder.jp/contests/abc094/tasks/abc094_b) | B - Toll Gates | 二分探索 | [abc094_b.rusk](abc094_b.rusk) | 3 |
| [ABC095 A](https://atcoder.jp/contests/abc095/tasks/abc095_a) | A - Something on It | 文字の計数 | [abc095_a.rusk](abc095_a.rusk) | 3 |
| [ABC095 B](https://atcoder.jp/contests/abc095/tasks/abc095_b) | B - Bitter Alchemy | 貪欲法 | [abc095_b.rusk](abc095_b.rusk) | 3 |
| [ABC096 B](https://atcoder.jp/contests/abc096/tasks/abc096_b) | B - Maximum Sum | 貪欲法 | [abc096_b.rusk](abc096_b.rusk) | 2 |
| [ABC097 B](https://atcoder.jp/contests/abc097/tasks/abc097_b) | B - Exponential | 整数の累乗 | [abc097_b.rusk](abc097_b.rusk) | 3 |
| [ABC098 B](https://atcoder.jp/contests/abc098/tasks/abc098_b) | B - Cut and Count | 文字の頻度配列 | [abc098_b.rusk](abc098_b.rusk) | 3 |
| [ABC099 B](https://atcoder.jp/contests/abc099/tasks/abc099_b) | B - Stone Monument | 等差数列 | [abc099_b.rusk](abc099_b.rusk) | 2 |
| [ABC100 B](https://atcoder.jp/contests/abc100/tasks/abc100_b) | B - Ringo's Favorite Numbers | 整数演算 | [abc100_b.rusk](abc100_b.rusk) | 3 |
| [ABC111 B](https://atcoder.jp/contests/abc111/tasks/abc111_b) | B - AtCoder Beginner Contest 111 | 整数の切り上げ | [abc111_b.rusk](abc111_b.rusk) | 3 |
| [ABC125 A](https://atcoder.jp/contests/abc125/tasks/abc125_a) | A - Biscuit Generator | 整数除算 | [abc125_a.rusk](abc125_a.rusk) | 3 |
| [ABC134 C](https://atcoder.jp/contests/abc134/tasks/abc134_c) | C - Exception Handling | ソートと最大値 | [abc134_c.rusk](abc134_c.rusk) | 2 |
| [ABC136 B](https://atcoder.jp/contests/abc136/tasks/abc136_b) | B - Uneven Numbers | 桁数の計数 | [abc136_b.rusk](abc136_b.rusk) | 3 |
| [ABC143 D](https://atcoder.jp/contests/abc143/tasks/abc143_d) | D - Triangles | ソートと尺取り法 | [abc143_d.rusk](abc143_d.rusk) | 3 |
| [ABC146 C](https://atcoder.jp/contests/abc146/tasks/abc146_c) | C - Buy an Integer | 答えの二分探索 | [abc146_c.rusk](abc146_c.rusk) | 4 |
| [ABC163 B](https://atcoder.jp/contests/abc163/tasks/abc163_b) | B - Homework | 配列の合計 | [abc163_b.rusk](abc163_b.rusk) | 4 |
| [ABC163 C](https://atcoder.jp/contests/abc163/tasks/abc163_c) | C - management | 頻度配列 | [abc163_c.rusk](abc163_c.rusk) | 3 |
| [ABC174 E](https://atcoder.jp/contests/abc174/tasks/abc174_e) | E - Logs | 答えの二分探索 | [abc174_e.rusk](abc174_e.rusk) | 3 |
| [ABC175 B](https://atcoder.jp/contests/abc175/tasks/abc175_b) | B - Making Triangle | 三重ループの全探索 | [abc175_b.rusk](abc175_b.rusk) | 4 |
| [ABC176 B](https://atcoder.jp/contests/abc176/tasks/abc176_b) | B - Multiple of 9 | 大きな整数の桁の和 | [abc176_b.rusk](abc176_b.rusk) | 3 |
| [ABC178 E](https://atcoder.jp/contests/abc178/tasks/abc178_e) | E - Dist Max | 座標変換と最大最小 | [abc178_e.rusk](abc178_e.rusk) | 2 |
| [ABC179 B](https://atcoder.jp/contests/abc179/tasks/abc179_b) | B - Go to Jail | 連続区間の計数 | [abc179_b.rusk](abc179_b.rusk) | 3 |
| [ABC179 C](https://atcoder.jp/contests/abc179/tasks/abc179_c) | C - A x B + C | 商による数え上げ | [abc179_c.rusk](abc179_c.rusk) | 3 |
| [ABC180 C](https://atcoder.jp/contests/abc180/tasks/abc180_c) | C - Cream puff | 約数列挙 | [abc180_c.rusk](abc180_c.rusk) | 3 |
| [ABC181 C](https://atcoder.jp/contests/abc181/tasks/abc181_c) | C - Collinearity | 外積と全探索 | [abc181_c.rusk](abc181_c.rusk) | 3 |
| [ABC184 B](https://atcoder.jp/contests/abc184/tasks/abc184_b) | B - Quizzes | シミュレーション | [abc184_b.rusk](abc184_b.rusk) | 3 |
| [ABC186 B](https://atcoder.jp/contests/abc186/tasks/abc186_b) | B - Blocks on Grid | 合計と最小値 | [abc186_b.rusk](abc186_b.rusk) | 3 |
| [ABC186 C](https://atcoder.jp/contests/abc186/tasks/abc186_c) | C - Unlucky 7 | 基数ごとの桁の検査 | [abc186_c.rusk](abc186_c.rusk) | 2 |
| [ABC187 B](https://atcoder.jp/contests/abc187/tasks/abc187_b) | B - Gentle Pairs | 整数での傾き判定 | [abc187_b.rusk](abc187_b.rusk) | 3 |
| [ABC188 B](https://atcoder.jp/contests/abc188/tasks/abc188_b) | B - Orthogonality | 内積 | [abc188_b.rusk](abc188_b.rusk) | 3 |
| [ABC190 B](https://atcoder.jp/contests/abc190/tasks/abc190_b) | B - Magic 3 | 条件の判定 | [abc190_b.rusk](abc190_b.rusk) | 3 |
| [ABC191 B](https://atcoder.jp/contests/abc191/tasks/abc191_b) | B - Remove It | 配列の抽出 | [abc191_b.rusk](abc191_b.rusk) | 2 |
| [ABC192 B](https://atcoder.jp/contests/abc192/tasks/abc192_b) | B - uNrEaDaBlE sTrInG | 文字の検査 | [abc192_b.rusk](abc192_b.rusk) | 3 |
| [ABC193 B](https://atcoder.jp/contests/abc193/tasks/abc193_b) | B - Play Snuke | 条件付き最小値 | [abc193_b.rusk](abc193_b.rusk) | 3 |
| [ABC194 B](https://atcoder.jp/contests/abc194/tasks/abc194_b) | B - Job Assignment | 割り当ての全探索 | [abc194_b.rusk](abc194_b.rusk) | 2 |
| [ABC194 C](https://atcoder.jp/contests/abc194/tasks/abc194_c) | C - Squared Error | 累積和と平方和 | [abc194_c.rusk](abc194_c.rusk) | 2 |
| [ABC196 C](https://atcoder.jp/contests/abc196/tasks/abc196_c) | C - Doubled | 候補の生成 | [abc196_c.rusk](abc196_c.rusk) | 3 |
| [ABC199 B](https://atcoder.jp/contests/abc199/tasks/abc199_b) | B - Intersection | 区間の共通部分 | [abc199_b.rusk](abc199_b.rusk) | 3 |
| [ABC200 B](https://atcoder.jp/contests/abc200/tasks/abc200_b) | B - 200th ABC-200 | シミュレーション | [abc200_b.rusk](abc200_b.rusk) | 3 |
| [ABC202 B](https://atcoder.jp/contests/abc202/tasks/abc202_b) | B - 180° | 文字列の変換 | [abc202_b.rusk](abc202_b.rusk) | 3 |
| [ABC205 B](https://atcoder.jp/contests/abc205/tasks/abc205_b) | B - Permutation Check | ソートと比較 | [abc205_b.rusk](abc205_b.rusk) | 4 |
| [ABC206 B](https://atcoder.jp/contests/abc206/tasks/abc206_b) | B - Savings | 累積和 | [abc206_b.rusk](abc206_b.rusk) | 2 |
| [ABC206 C](https://atcoder.jp/contests/abc206/tasks/abc206_c) | C - Swappable | ソートと同値区間 | [abc206_c.rusk](abc206_c.rusk) | 3 |
| [ABC208 B](https://atcoder.jp/contests/abc208/tasks/abc208_b) | B - Factorial Yen Coin | 階乗と貪欲法 | [abc208_b.rusk](abc208_b.rusk) | 3 |
| [ABC211 B](https://atcoder.jp/contests/abc211/tasks/abc211_b) | B - Cycle Hit | 配列の検索 | [abc211_b.rusk](abc211_b.rusk) | 2 |
| [ABC213 B](https://atcoder.jp/contests/abc213/tasks/abc213_b) | B - Booby Prize | ソートと元の位置 | [abc213_b.rusk](abc213_b.rusk) | 2 |
| [ABC214 B](https://atcoder.jp/contests/abc214/tasks/abc214_b) | B - How many? | 三重ループの全探索 | [abc214_b.rusk](abc214_b.rusk) | 4 |
| [ABC215 B](https://atcoder.jp/contests/abc215/tasks/abc215_b) | B - log2(N) | 整数の対数 | [abc215_b.rusk](abc215_b.rusk) | 3 |
| [ABC217 B](https://atcoder.jp/contests/abc217/tasks/abc217_b) | B - AtCoder Quiz | 配列の検索 | [abc217_b.rusk](abc217_b.rusk) | 2 |
| [ABC221 B](https://atcoder.jp/contests/abc221/tasks/abc221_b) | B - typo | 隣接交換の全探索 | [abc221_b.rusk](abc221_b.rusk) | 3 |
| [ABC226 B](https://atcoder.jp/contests/abc226/tasks/abc226_b) | B - Counting Arrays | 配列のソートと重複除去 | [abc226_b.rusk](abc226_b.rusk) | 3 |
| [ABC227 B](https://atcoder.jp/contests/abc227/tasks/abc227_b) | B - KEYENCE building | 候補の前計算 | [abc227_b.rusk](abc227_b.rusk) | 2 |
| [ABC228 B](https://atcoder.jp/contests/abc228/tasks/abc228_b) | B - Takahashi's Secret | 訪問済み配列 | [abc228_b.rusk](abc228_b.rusk) | 2 |
| [ABC229 B](https://atcoder.jp/contests/abc229/tasks/abc229_b) | B - Hard Calculation | 桁ごとの計算 | [abc229_b.rusk](abc229_b.rusk) | 2 |
| [ABC232 B](https://atcoder.jp/contests/abc232/tasks/abc232_b) | B - Caesar Cipher | 文字コードと剰余 | [abc232_b.rusk](abc232_b.rusk) | 4 |
| [ABC233 B](https://atcoder.jp/contests/abc233/tasks/abc233_b) | B - A Reverse | 両端からの交換 | [abc233_b.rusk](abc233_b.rusk) | 3 |
| [ABC235 B](https://atcoder.jp/contests/abc235/tasks/abc235_b) | B - Climbing Takahashi | 単調な区間 | [abc235_b.rusk](abc235_b.rusk) | 3 |
| [ABC236 B](https://atcoder.jp/contests/abc236/tasks/abc236_b) | B - Who is missing? | 頻度配列 | [abc236_b.rusk](abc236_b.rusk) | 3 |
| [ABC237 C](https://atcoder.jp/contests/abc237/tasks/abc237_c) | C - kasaka | 両端からの走査 | [abc237_c.rusk](abc237_c.rusk) | 3 |
| [ABC238 B](https://atcoder.jp/contests/abc238/tasks/abc238_b) | B - Pizza | 角度の累積とソート | [abc238_b.rusk](abc238_b.rusk) | 3 |
| [ABC239 B](https://atcoder.jp/contests/abc239/tasks/abc239_b) | B - Integer Division | 負数の整数除算 | [abc239_b.rusk](abc239_b.rusk) | 5 |
| [ABC240 C](https://atcoder.jp/contests/abc240/tasks/abc240_c) | C - Jumping Takahashi | 到達可能性のDP | [abc240_c.rusk](abc240_c.rusk) | 3 |
| [ABC241 B](https://atcoder.jp/contests/abc241/tasks/abc241_b) | B - Pasta | ソートと二つのポインター | [abc241_b.rusk](abc241_b.rusk) | 3 |
| [ABC244 B](https://atcoder.jp/contests/abc244/tasks/abc244_b) | B - Go Straight and Turn Right | 方向とシミュレーション | [abc244_b.rusk](abc244_b.rusk) | 2 |
| [ABC248 B](https://atcoder.jp/contests/abc248/tasks/abc248_b) | B - Slimes | 乗算の反復 | [abc248_b.rusk](abc248_b.rusk) | 3 |
| [ABC249 B](https://atcoder.jp/contests/abc249/tasks/abc249_b) | B - Perfect String | 文字の頻度配列 | [abc249_b.rusk](abc249_b.rusk) | 4 |
| [ABC250 B](https://atcoder.jp/contests/abc250/tasks/abc250_b) | B - Enlarged Checker Board | グリッドの生成 | [abc250_b.rusk](abc250_b.rusk) | 4 |
| [ABC252 B](https://atcoder.jp/contests/abc252/tasks/abc252_b) | B - Takahashi's Failure | 最大値と検索 | [abc252_b.rusk](abc252_b.rusk) | 3 |
| [ABC256 B](https://atcoder.jp/contests/abc256/tasks/abc256_b) | B - Batters | 固定長配列のシミュレーション | [abc256_b.rusk](abc256_b.rusk) | 3 |
| [ABC257 B](https://atcoder.jp/contests/abc257/tasks/abc257_b) | B - 1D Pawn | 隣接する駒のシミュレーション | [abc257_b.rusk](abc257_b.rusk) | 3 |
| [ABC258 B](https://atcoder.jp/contests/abc258/tasks/abc258_b) | B - Number Box | 八方向の全探索 | [abc258_b.rusk](abc258_b.rusk) | 2 |
| [ABC261 B](https://atcoder.jp/contests/abc261/tasks/abc261_b) | B - Tournament Result | 行列の整合性検査 | [abc261_b.rusk](abc261_b.rusk) | 2 |
| [ABC263 B](https://atcoder.jp/contests/abc263/tasks/abc263_b) | B - Ancestor | 親の走査 | [abc263_b.rusk](abc263_b.rusk) | 2 |
| [ABC265 B](https://atcoder.jp/contests/abc265/tasks/abc265_b) | B - Explore | シミュレーションとボーナス配列 | [abc265_b.rusk](abc265_b.rusk) | 2 |
| [ABC271 B](https://atcoder.jp/contests/abc271/tasks/abc271_b) | B - Maintain Multiple Sequences | 多次元配列とクエリ | [abc271_b.rusk](abc271_b.rusk) | 2 |
| [ABC272 B](https://atcoder.jp/contests/abc272/tasks/abc272_b) | B - Everyone is Friends | 隣接行列 | [abc272_b.rusk](abc272_b.rusk) | 2 |
| [ABC289 B](https://atcoder.jp/contests/abc289/tasks/abc289_b) | B - V | 連結区間の反転 | [abc289_b.rusk](abc289_b.rusk) | 3 |
| [ABC293 B](https://atcoder.jp/contests/abc293/tasks/abc293_b) | B - Call the ID Number | 状態配列 | [abc293_b.rusk](abc293_b.rusk) | 2 |
| [ABC305 C](https://atcoder.jp/contests/abc305/tasks/abc305_c) | C - Snuke the Cookie Picker | グリッドの境界 | [abc305_c.rusk](abc305_c.rusk) | 3 |
| [ABC320 B](https://atcoder.jp/contests/abc320/tasks/abc320_b) | B - Longest Palindrome | 関数と回文判定 | [abc320_b.rusk](abc320_b.rusk) | 3 |
| [ABC332 B](https://atcoder.jp/contests/abc332/tasks/abc332_b) | B - Glass and Mug | シミュレーション | [abc332_b.rusk](abc332_b.rusk) | 2 |
| [ABC373 B](https://atcoder.jp/contests/abc373/tasks/abc373_b) | B - 1D Keyboard | 文字の位置配列 | [abc373_b.rusk](abc373_b.rusk) | 2 |
| [ABC373 C](https://atcoder.jp/contests/abc373/tasks/abc373_c) | C - Max Ai+Bj | 最大値 | [abc373_c.rusk](abc373_c.rusk) | 2 |
