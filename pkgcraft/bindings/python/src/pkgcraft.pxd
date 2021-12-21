# SPDX-License-Identifier: MIT
# generated with cbindgen
# cython: language_level=3

from libc.stdint cimport int8_t, int16_t, int32_t, int64_t, intptr_t
from libc.stdint cimport uint8_t, uint16_t, uint32_t, uint64_t, uintptr_t
cdef extern from *:
    ctypedef bint bool
    ctypedef struct va_list

cdef extern from "pkgcraft.h":

    cdef struct Atom:
        const char *category;
        const char *package;
        const char *version;
        const char *slot;
        const char *subslot;
        const char *const *use_deps;
        uintptr_t use_deps_len;
        const char *repo;

    # Parse a string into an atom using a specific EAPI. Pass a null pointer for the eapi argument in
    # order to parse using the latest EAPI with extensions (e.g. support for repo deps).
    Atom *str_to_atom(const char *atom, const char *eapi);

    # Free atom object.
    void atom_free(Atom *atom);

    # Get the most recent error message as a UTF-8 string, if none exists a null pointer is returned.
    #
    # The caller is expected to free memory used by the string after they're finished using it.
    char *last_error_message();
