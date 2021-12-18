from setuptools import Extension, setup
from Cython.Build import cythonize

extensions = [Extension("pkgcraft", ["src/pkgcraft.pyx"], libraries=["pkgcraft"])]

setup(
    name='python bindings for pkgcraft',
    ext_modules=cythonize(extensions),
)
