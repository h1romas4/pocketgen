/* Native-sim's Rust target uses a different floating-point ABI from C. */
#define POCKETGEN_MATH_UNARY(name) \
    extern double pocketgen_host_##name(double); \
    double name(double x) { return pocketgen_host_##name(x); }
#define POCKETGEN_MATH_BINARY(name) \
    extern double pocketgen_host_##name(double, double); \
    double name(double x, double y) { return pocketgen_host_##name(x, y); }
#include "native_math_functions.h"
#undef POCKETGEN_MATH_UNARY
#undef POCKETGEN_MATH_BINARY

extern double pocketgen_host_frexp(double, int *);
extern double pocketgen_host_ldexp(double, int);
extern double pocketgen_host_modf(double, double *);
double frexp(double x, int *exponent) { return pocketgen_host_frexp(x, exponent); }
double ldexp(double x, int exponent) { return pocketgen_host_ldexp(x, exponent); }
double modf(double x, double *integer) { return pocketgen_host_modf(x, integer); }
