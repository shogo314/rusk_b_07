#include <unordered_map>
#include <unordered_set>
#include <functional>
#include <chrono>
#include <variant>
// Empty-or-one-shared-value enums use the null pointer as their empty tag.
// This is a layout optimization for any enum of that shape, including Option.
template<class E,class S> std::size_t rusk_enum_index(const S& value){
    if constexpr(E::nullable) return value ? E::present : E::absent;
    else return value.index();
}
template<class S> struct RuskEnumPayloadView { S& field_0; };
template<class E,std::size_t I,class S> decltype(auto) rusk_enum_payload(S& value){
    if constexpr(E::nullable){
        static_assert(I==E::present);
        return RuskEnumPayloadView<S>{value};
    }else return (std::get<I>(value));
}
template<class E,std::size_t I,class... A> Obj<E> rusk_enum_make(A&&... args){
    if constexpr(E::nullable){
        if constexpr(I==E::present){
            static_assert(sizeof...(A)==1);
            return make_obj<E>(E{std::forward<A>(args)...});
        }else{
            static_assert(I==E::absent && sizeof...(A)==0);
            return make_obj<E>(E{});
        }
    }else{
        using V=std::variant_alternative_t<I,decltype(E::value)>;
        return make_obj<E>(E{decltype(E::value){std::in_place_index<I>,V{std::forward<A>(args)...}}});
    }
}
template<class T> std::size_t rusk_hash_value(const Obj<T>& x) { return std::hash<T>{}(*x); }
inline void rusk_hash_combine(std::size_t& hash,std::size_t part) {hash^=part+std::size_t(0x9e3779b9)+(hash<<6)+(hash>>2);}
inline std::uint64_t rusk_mix_hash(std::uint64_t x) {
    x += UINT64_C(0x9e3779b97f4a7c15);
    x = (x ^ (x >> 30)) * UINT64_C(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)) * UINT64_C(0x94d049bb133111eb);
    return x ^ (x >> 31);
}
inline std::uint64_t rusk_hash_seed() {
    static const auto seed = static_cast<std::uint64_t>(std::chrono::steady_clock::now().time_since_epoch().count());
    return seed;
}
template<class T> struct RuskHash {
    std::size_t operator()(const Obj<T>& x) const {
        return static_cast<std::size_t>(rusk_mix_hash(static_cast<std::uint64_t>(rusk_hash_value(x)) + rusk_hash_seed()));
    }
};
template<class T> struct RuskEqual {bool operator()(const Obj<T>& a,const Obj<T>& b) const{return value_equal(a,b);}};
template<class K,class V> struct RuskDict {
    using Entries=std::unordered_map<Obj<K>,Obj<V>,RuskHash<K>,RuskEqual<K>>;
    Entries entries;
    std::uint64_t version=0;
};
template<class K> struct RuskSet {
    using Entries=std::unordered_set<Obj<K>,RuskHash<K>,RuskEqual<K>>;
    Entries entries;
    std::uint64_t version=0;
};
