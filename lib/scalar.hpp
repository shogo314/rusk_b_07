// Closed scalar groups have no object escapes except their final return.
// Their aliases refer to this one value. Conversion is used only by return,
// where a new owning object is created before local cleanup.
template<class T> class RuskInlineScalar {
    T value{};
public:
    RuskInlineScalar() = default;
    explicit RuskInlineScalar(T initial) : value(std::move(initial)) {}
    RuskInlineScalar(const RuskInlineScalar&) = delete;
    RuskInlineScalar& operator=(const RuskInlineScalar&) = delete;
    RuskInlineScalar(RuskInlineScalar&&) = default;
    RuskInlineScalar& operator=(RuskInlineScalar&&) = default;
    T& operator*() { return value; }
    const T& operator*() const { return value; }
    operator Obj<T>() const { return make_obj<T>(value); }
    void reset() {}
};

// Fresh scalar expression results start inline. A shared object is created on
// the first escape into a variable, aggregate, or ordinary function argument.
// Subsequent reads and writes use that SAME object, including after promotion.
template<class T> class RuskTemporary {
    T value;
    mutable Obj<T> shared;
public:
    RuskTemporary() : value{} {}
    explicit RuskTemporary(T initial) : value(std::move(initial)) {}
    RuskTemporary(const RuskTemporary&) = delete;
    RuskTemporary& operator=(const RuskTemporary&) = delete;
    RuskTemporary(RuskTemporary&&) = default;
    RuskTemporary& operator=(RuskTemporary&&) = default;
    operator Obj<T>() const {
        if (!shared) shared = make_obj<T>(value);
        return shared;
    }
    T& operator*() { return shared ? *shared : value; }
    const T& operator*() const { return shared ? *shared : value; }
    void reset() { shared.reset(); }
};
inline std::int64_t rusk_div_int(std::int64_t a, std::int64_t b, Loc loc) {
    if (b == 0) fail(loc, "division by zero");
    __int128 a0 = a, b0 = b;
    __int128 q = a0 / b0, r = a0 % b0;
    if (r != 0 && ((r < 0) != (b0 < 0))) --q;
    return checked_int(q, loc);
}
inline std::int64_t rusk_mod_int(std::int64_t a, std::int64_t b, Loc loc) {
    if (b == 0) fail(loc, "division by zero");
    __int128 a0 = a, b0 = b;
    __int128 r = a0 % b0;
    if (r != 0 && ((r < 0) != (b0 < 0))) r += b0;
    return checked_int(r, loc);
}
inline std::int64_t rusk_read_int(Loc loc) {
    auto token = read_token(loc);
    const char* first = token.data();
    const char* last = first + token.size();
    if (first != last && *first == '+') {
        ++first;
        if (first == last || *first < '0' || *first > '9') fail(loc, "invalid integer input");
    }
    std::int64_t value = 0;
    auto result = std::from_chars(first, last, value);
    if (result.ec == std::errc::result_out_of_range) fail(loc, "integer input out of range");
    if (result.ec != std::errc{} || result.ptr != last) fail(loc, "invalid integer input");
    return value;
}
// RuskRangeIter is declared by the collections runtime, after this header.
// Keep the dependent member access in a template until that type is complete.
template<class Iterator> std::int64_t rusk_range_next(const Obj<Iterator>& it, Loc loc) {
    if (it->next >= it->end) fail(loc, "iterator exhausted");
    auto value = it->next;
    ++it->next;
    return value;
}
