# Re-implementation of Basis Set Exchange (code part only) in Rust

**This is currently a stub. This program has not been and not ready to be published.**

> BSE? As a programmer, I don't understand Bethe-Salpeter Equation very well.

Reimplementation of python library and CLI [Basis Set Exchange](https://github.com/MolSSI-BSE/basis_set_exchange/) in rust.

## Motivation

Nowadays, many computational chemistry infrastructures are implemented in Python.

As a computational chemistry developer (previously extensively uses PySCF), I've also benefitted a lot from those quickly developed open-source infrastructures.

However, when coming to compiled language, Python then is not the best choice. Python is good at gluing other languages to it, not from it.
Even rust is one of best languages to call python (by PyO3 framework), it still have very strong limitation that PyO3 must link `libpython3.*.so` at compile time, not convenient on consideration of convenient binary distribution.

Since I'm currently working on rust, I (potentially) need a rust library. If no such kind of library (are there even 1% chemistry developers using rust?), then C-compatiable APIs seems to be a very important issue for infrastructures.

Anyway, since REST currently can handle json basis (format similar to BSE but not exactly the same), so a BSE in rust is actually not at a high priority from REST's perspective, and this project is still a hobby project.

## Current status

This project is currently a hobby part-time project.

Features implemented:
- Full functionality of `get_basis` and `get_formatted_basis` function (json output, writers, optional parameters).
    - Writers include `nwchem`, `turbomole`, `orca`, `gaussian94`, `psi4`, etc. Almost all available writers implemented by python BSE is also realized in rust BSE.
    - Optional parame/ters include uncontraction, auto-aux generation, etc.
- Partial functionality of `read_formatted_basis_str` function
    - Readers currently only includes `nwchem`, `gaussian94`. Many other readers are not performing well in ECP reading for python BSE, so we also not implementing these readers.

With AI (deepseek) assistance, those features are implemented in about 4-6 days. AI prompts is available at directory [deepseek_hints].

Implemneted features are provided **AS IS, including potential bugs from python BSE**.

Features to be implemented:
- Reference support
- CLI support
- C language API support
- website support
- PyO3 support (re-export rust package to Python, sounds weird since original BSE is already a python lib)

Currently this project just uses python's BSE basis set data. Data maintainance is not an aim (which is the most difficult part of python's BSE maintainance, I believe).
