# Interface wrapper for pkgcraft library.
# cython: language_level=3

cdef extern from "pkgcraft.h":
    struct Atom:
        const char *category
        const char *package
        const char *version
        const char *slot
        const char *subslot
        const char *repo

    Atom *str_to_atom(const char *s)
    void atom_free(Atom *atom)
