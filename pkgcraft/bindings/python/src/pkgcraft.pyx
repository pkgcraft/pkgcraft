# SPDX-License-Identifier: MIT
# cython: language_level=3

from libc.stdlib cimport free

cimport pkgcraft

cdef class PkgcraftException(Exception):
    cdef char *_error

    def __init__(self, str msg):
        self._error = pkgcraft.last_error_message()
        if self._error is not NULL:
            super().__init__(self._error.decode())
        else:
            super().__init__(msg)

    def __dealloc__(self):
        if self._error is not NULL:
            free(self._error)

cdef class atom:
    cdef pkgcraft.Atom *_atom
    cdef public str category
    cdef public str package
    cdef public str version
    cdef public str slot
    cdef public str subslot
    cdef public tuple use_deps
    cdef public str repo

    def __init__(self, str atom, str eapi=None):
        if eapi is None:
            self._atom = pkgcraft.str_to_atom(atom.encode(), NULL)
        else:
            self._atom = pkgcraft.str_to_atom(atom.encode(), eapi.encode())

        if self._atom is NULL:
            raise PkgcraftException("invalid atom")

        self.category = self._atom.category.decode()
        self.package = self._atom.package.decode()

        if self._atom.version is not NULL:
            self.version = self._atom.version.decode()
        else:
            self.version = None

        if self._atom.slot is not NULL:
            self.slot = self._atom.slot.decode()
        else:
            self.slot = None

        if self._atom.subslot is not NULL:
            self.subslot = self._atom.subslot.decode()
        else:
            self.subslot = None

        if self._atom.use_deps_len:
            self.use_deps = tuple(
                self._atom.use_deps[i].decode() for i in range(self._atom.use_deps_len))
        else:
            self.use_deps = None

        if self._atom.repo is not NULL:
            self.repo = self._atom.repo.decode()
        else:
            self.repo = None

    def key(self):
        cdef const char *key_str = pkgcraft.atom_key(self._atom)
        cdef str key = key_str.decode()
        free(<void *>key_str)
        return key

    def cpv(self):
        cdef const char *cpv_str = pkgcraft.atom_cpv(self._atom)
        cdef str cpv = cpv_str.decode()
        free(<void *>cpv_str)
        return cpv

    def __dealloc__(self):
        pkgcraft.atom_free(self._atom)
