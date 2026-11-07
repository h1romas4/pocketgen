#include <math.h>

#define POCKETGEN_MATH_UNARY(name) \
    double pocketgen_host_##name(double x) { return name(x); }
#define POCKETGEN_MATH_BINARY(name) \
    double pocketgen_host_##name(double x, double y) { return name(x, y); }
#include "native_math_functions.h"
#undef POCKETGEN_MATH_UNARY
#undef POCKETGEN_MATH_BINARY

double pocketgen_host_frexp(double x, int *exponent) { return frexp(x, exponent); }
double pocketgen_host_ldexp(double x, int exponent) { return ldexp(x, exponent); }
double pocketgen_host_modf(double x, double *integer) { return modf(x, integer); }
