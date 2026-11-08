// Buffered standard streams. Native helpers and extern std::cin/std::cout use
// the same stream buffers, so mixing the two does not lose prefetched bytes.
#include <algorithm>
#include <cstdio>
#include <cstring>
#include <streambuf>
struct RuskInputBuffer : std::streambuf {
    char bytes[65536];
    int_type underflow() override {
        auto count = std::fread(bytes, 1, sizeof(bytes), stdin);
        if (count == 0) return traits_type::eof();
        setg(bytes, bytes, bytes + count);
        return traits_type::to_int_type(*gptr());
    }
};
struct RuskOutputBuffer : std::streambuf {
    char bytes[65536];
    RuskOutputBuffer() { setp(bytes, bytes + sizeof(bytes)); }
    int sync() override {
        auto count = static_cast<std::size_t>(pptr() - pbase());
        if (count && std::fwrite(pbase(), 1, count, stdout) != count) return -1;
        setp(bytes, bytes + sizeof(bytes));
        return std::fflush(stdout);
    }
    int_type overflow(int_type c) override {
        if (sync() != 0) return traits_type::eof();
        if (!traits_type::eq_int_type(c, traits_type::eof())) {
            *pptr() = traits_type::to_char_type(c);
            pbump(1);
        }
        return traits_type::not_eof(c);
    }
    std::streamsize xsputn(const char* source, std::streamsize count) override {
        std::streamsize done = 0;
        while (done < count) {
            if (pptr() == epptr() && sync() != 0) break;
            auto size = std::min<std::streamsize>(epptr() - pptr(), count - done);
            std::memcpy(pptr(), source + done, static_cast<std::size_t>(size));
            pbump(static_cast<int>(size));
            done += size;
        }
        return done;
    }
};
struct RuskIO {
    RuskInputBuffer input;
    RuskOutputBuffer output;
    std::streambuf* previous_input;
    std::streambuf* previous_output;
    RuskIO() {
        std::ios::sync_with_stdio(false);
        std::cin.tie(nullptr);
        previous_input = std::cin.rdbuf(&input);
        previous_output = std::cout.rdbuf(&output);
    }
    ~RuskIO() {
        std::cout.flush();
        std::fflush(stdout);
        std::cout.rdbuf(previous_output);
        std::cin.rdbuf(previous_input);
    }
};
inline RuskIO rusk_io;
inline int rusk_peek() { return rusk_io.input.sgetc(); }
inline int rusk_get() { return rusk_io.input.sbumpc(); }
inline void rusk_write(char c) { rusk_io.output.sputc(c); }
inline void rusk_write(const std::string& s) {
    rusk_io.output.sputn(s.data(), static_cast<std::streamsize>(s.size()));
}
inline void rusk_write(std::int64_t n) {
    char bytes[32];
    auto end = std::to_chars(bytes, bytes + sizeof(bytes), n).ptr;
    rusk_io.output.sputn(bytes, end - bytes);
}
inline void rusk_write(bool value) {
    if (value) rusk_io.output.sputn("Yes", 3);
    else rusk_io.output.sputn("No", 2);
}
template<class T> void rusk_write(const T& value) { std::cout << value; }
inline std::string rusk_read_line(Loc loc) {
    std::string value;
    int c = rusk_get();
    if (c == std::char_traits<char>::eof()) fail(loc, "unexpected end of input");
    while (c != '\n' && c != std::char_traits<char>::eof()) {
        value.push_back(static_cast<char>(c));
        c = rusk_get();
    }
    if (!value.empty() && value.back() == '\r') value.pop_back();
    return value;
}
