cimport pkgcraft

cdef class atom:
    cdef pkgcraft.Atom *_atom
    cdef public str category
    cdef public str package
    cdef public str version

    def __init__(self, str s):
        self._atom = pkgcraft.str_to_atom(s.encode())
        self.category = self._atom.category.decode()
        self.package = self._atom.package.decode()
        if self._atom.version is not NULL:
            self.version = self._atom.version.decode()
        else:
            self.version = None

    def __dealloc__(self):
        pkgcraft.atom_free(self._atom)
