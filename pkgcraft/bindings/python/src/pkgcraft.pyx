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
        self.version = self._atom.version.decode()

    def __dealloc__(self):
        pkgcraft.atom_free(self._atom)
