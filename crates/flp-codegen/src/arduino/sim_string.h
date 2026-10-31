// Arduino String for simulation modules: fixed capacity, no libc, no heap.
#define FLP_STRING_CAP 256

extern "C" {
FLP(input_text) int input_text(int index, char *ptr, int cap);
FLP(param_text) int param_text(int index, char *ptr, int cap);
FLP(output_text) void output_text(int index, const char *ptr, int len);
}

struct String {
  char b[FLP_STRING_CAP + 1];
  unsigned n;

  String() : n(0) { b[0] = 0; }
  String(const char *s) : n(0) { b[0] = 0; if (s) append(s, flp_len(s)); }
  String(char c) : n(0) { b[0] = 0; append(&c, 1); }
  String(int v) : n(0) { b[0] = 0; put_int((long long)v); }
  String(unsigned v) : n(0) { b[0] = 0; put_uint((unsigned long long)v); }
  String(long v) : n(0) { b[0] = 0; put_int((long long)v); }
  String(unsigned long v) : n(0) { b[0] = 0; put_uint((unsigned long long)v); }
  String(float v, int decimals = 2) : n(0) { b[0] = 0; put_float((double)v, decimals); }
  String(double v, int decimals = 2) : n(0) { b[0] = 0; put_float(v, decimals); }

  static unsigned flp_len(const char *s) { unsigned k = 0; while (s[k]) k++; return k; }

  void append(const char *s, unsigned len) {
    for (unsigned i = 0; i < len && n < FLP_STRING_CAP; i++) b[n++] = s[i];
    b[n] = 0;
  }
  void put_uint(unsigned long long v) {
    char t[24]; int k = 0;
    do { t[k++] = (char)('0' + v % 10); v /= 10; } while (v);
    while (k) append(&t[--k], 1);
  }
  void put_int(long long v) {
    if (v < 0) { append("-", 1); put_uint((unsigned long long)(-v)); } else put_uint((unsigned long long)v);
  }
  void put_float(double v, int decimals) {
    if (v != v) { append("nan", 3); return; }
    if (v < 0) { append("-", 1); v = -v; }
    if (decimals < 0) decimals = 0;
    if (decimals > 9) decimals = 9;
    double scale = 1;
    for (int i = 0; i < decimals; i++) scale *= 10;
    double r = v * scale + 0.5;
    if (r > 1.8e19) { append("ovf", 3); return; }
    unsigned long long all = (unsigned long long)r;
    unsigned long long whole = all / (unsigned long long)scale;
    unsigned long long frac = all % (unsigned long long)scale;
    put_uint(whole);
    if (decimals) {
      append(".", 1);
      char t[10];
      for (int i = decimals - 1; i >= 0; i--) { t[i] = (char)('0' + frac % 10); frac /= 10; }
      append(t, (unsigned)decimals);
    }
  }

  unsigned length() const { return n; }
  const char *c_str() const { return b; }
  char operator[](unsigned i) const { return i < n ? b[i] : 0; }
  char charAt(unsigned i) const { return (*this)[i]; }

  String &operator+=(const String &o) { append(o.b, o.n); return *this; }
  String &operator+=(const char *s) { append(s, flp_len(s)); return *this; }
  String &operator+=(char c) { append(&c, 1); return *this; }
  String operator+(const String &o) const { String r(*this); r += o; return r; }
  String operator+(const char *s) const { String r(*this); r += s; return r; }
  String operator+(char c) const { String r(*this); r += c; return r; }

  bool operator==(const String &o) const {
    if (n != o.n) return false;
    for (unsigned i = 0; i < n; i++) if (b[i] != o.b[i]) return false;
    return true;
  }
  bool operator==(const char *s) const { return *this == String(s); }
  bool operator!=(const String &o) const { return !(*this == o); }
  bool operator!=(const char *s) const { return !(*this == String(s)); }

  int indexOf(const String &s, unsigned from = 0) const {
    if (from > n) return -1;
    for (unsigned i = from; i + s.n <= n; i++) {
      unsigned k = 0;
      while (k < s.n && b[i + k] == s.b[k]) k++;
      if (k == s.n) return (int)i;
    }
    return -1;
  }
  int indexOf(char c, unsigned from = 0) const { return indexOf(String(c), from); }
  int lastIndexOf(char c) const {
    for (int i = (int)n - 1; i >= 0; i--) if (b[i] == c) return i;
    return -1;
  }
  bool startsWith(const String &s) const {
    if (s.n > n) return false;
    for (unsigned i = 0; i < s.n; i++) if (b[i] != s.b[i]) return false;
    return true;
  }
  bool endsWith(const String &s) const {
    if (s.n > n) return false;
    for (unsigned i = 0; i < s.n; i++) if (b[n - s.n + i] != s.b[i]) return false;
    return true;
  }
  String substring(unsigned from, unsigned to) const {
    if (from > to) { unsigned t = from; from = to; to = t; }
    if (from >= n) return String();
    if (to > n) to = n;
    String r;
    r.append(b + from, to - from);
    return r;
  }
  String substring(unsigned from) const { return substring(from, n); }
  void toLowerCase() { for (unsigned i = 0; i < n; i++) if (b[i] >= 'A' && b[i] <= 'Z') b[i] = (char)(b[i] + 32); }
  void toUpperCase() { for (unsigned i = 0; i < n; i++) if (b[i] >= 'a' && b[i] <= 'z') b[i] = (char)(b[i] - 32); }
  void trim() {
    unsigned s = 0, e = n;
    while (s < e && (b[s] == ' ' || b[s] == '\t' || b[s] == '\r' || b[s] == '\n')) s++;
    while (e > s && (b[e - 1] == ' ' || b[e - 1] == '\t' || b[e - 1] == '\r' || b[e - 1] == '\n')) e--;
    for (unsigned i = s; i < e; i++) b[i - s] = b[i];
    n = e - s;
    b[n] = 0;
  }
  void replace(char from, char to) { for (unsigned i = 0; i < n; i++) if (b[i] == from) b[i] = to; }
  void replace(const String &from, const String &to) {
    if (from.n == 0) return;
    String r;
    unsigned i = 0;
    while (i < n) {
      if (i + from.n <= n && substring(i, i + from.n) == from) { r += to; i += from.n; }
      else { r += b[i]; i++; }
    }
    *this = r;
  }
  void remove(unsigned from) { if (from < n) { n = from; b[n] = 0; } }
  void remove(unsigned from, unsigned count) {
    if (from >= n) return;
    if (from + count > n) count = n - from;
    for (unsigned i = from; i + count < n; i++) b[i] = b[i + count];
    n -= count;
    b[n] = 0;
  }
  void concat(const String &o) { *this += o; }
  long toInt() const {
    unsigned i = 0; bool neg = false; long v = 0;
    while (i < n && b[i] == ' ') i++;
    if (i < n && (b[i] == '-' || b[i] == '+')) { neg = b[i] == '-'; i++; }
    while (i < n && b[i] >= '0' && b[i] <= '9') { v = v * 10 + (b[i] - '0'); i++; }
    return neg ? -v : v;
  }
  float toFloat() const {
    unsigned i = 0; bool neg = false; double v = 0;
    while (i < n && b[i] == ' ') i++;
    if (i < n && (b[i] == '-' || b[i] == '+')) { neg = b[i] == '-'; i++; }
    while (i < n && b[i] >= '0' && b[i] <= '9') { v = v * 10 + (b[i] - '0'); i++; }
    if (i < n && b[i] == '.') {
      i++;
      double k = 0.1;
      while (i < n && b[i] >= '0' && b[i] <= '9') { v += (b[i] - '0') * k; k /= 10; i++; }
    }
    return (float)(neg ? -v : v);
  }
};

static String flp_input_text(int index) {
  String s;
  s.n = (unsigned)input_text(index, s.b, FLP_STRING_CAP);
  if (s.n > FLP_STRING_CAP) s.n = FLP_STRING_CAP;
  s.b[s.n] = 0;
  return s;
}
static String flp_param_text(int index) {
  String s;
  s.n = (unsigned)param_text(index, s.b, FLP_STRING_CAP);
  if (s.n > FLP_STRING_CAP) s.n = FLP_STRING_CAP;
  s.b[s.n] = 0;
  return s;
}
static void flp_output_text(int index, const String &s) { output_text(index, s.b, (int)s.n); }

// Minimal strtod for sim modules: sign, digits, fraction; no exponent.
static double strtod(const char *s, char **end) {
  const char *p = s;
  bool neg = false;
  while (*p == ' ') p++;
  if (*p == '-' || *p == '+') { neg = *p == '-'; p++; }
  const char *digits = p;
  double v = 0;
  while (*p >= '0' && *p <= '9') { v = v * 10 + (*p - '0'); p++; }
  if (*p == '.') {
    p++;
    double k = 0.1;
    while (*p >= '0' && *p <= '9') { v += (*p - '0') * k; k /= 10; p++; }
  }
  if (p == digits) p = s;
  if (end) *end = (char *)p;
  return neg ? -v : v;
}
