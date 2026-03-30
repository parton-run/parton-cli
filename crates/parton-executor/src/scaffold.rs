//! Scaffold execution — generate minimal compilable stubs.
//!
//! Produces files that compile and have correct exports/imports,
//! but with stub implementations.

/// System prompt for scaffold (skeleton) execution.
pub const SCAFFOLD_PROMPT: &str = "\
You are a file scaffolder. You produce MINIMAL compilable file stubs.

YOUR OUTPUT FORMAT:
Line 1: ===FILE_START===
Lines 2..N: the MINIMAL source code
Last line: ===FILE_END===

RULES:
1. Output the MINIMUM code needed to make the file compile and satisfy all exports.
2. Functions: correct signature, return a placeholder value (empty string, 0, null, empty array).
3. Types/interfaces: complete definition with all fields and correct types.
4. Components: render a minimal div with the component name. Include all props in the signature.
5. Hooks: return the correct shape with placeholder values and stub functions.
6. Config files: complete and correct configuration — these ARE the final version.
7. Test files: single placeholder test (describe + it with expect(true).toBe(true)).
8. CSS files: complete with all required imports/directives — these ARE the final version.
9. HTML files: complete entry point — these ARE the final version.
10. Export ALL symbols listed in MANDATORY Exports with EXACT names.
11. Import ALL symbols from Import Interfaces with EXACT names.
12. The goal is COMPILABLE STRUCTURE, not functionality.
13. NEVER use markdown fences. Just raw code between markers.";

/// System prompt for final execution — full implementation preserving structure.
pub const FINAL_PROMPT: &str = "\
You are a file implementer. You receive a WORKING scaffold file and a detailed goal.
Your job is to replace stub implementations with real, complete code.

YOUR OUTPUT FORMAT:
Line 1: ===FILE_START===
Lines 2..N: the COMPLETE implemented source code
Last line: ===FILE_END===

CRITICAL RULES:
1. PRESERVE all import statements EXACTLY as they are in the scaffold. Do NOT rename, reorder, or remove any imports.
2. PRESERVE all export names EXACTLY. The same symbols must be exported with the same names.
3. REPLACE stub/placeholder implementations with real, working code.
4. The file must remain compilable after your changes.
5. If the scaffold file is already complete (config files, CSS, HTML), output it UNCHANGED.
6. For test files: replace placeholder tests with real tests covering the described functionality.
7. NEVER change function signatures — same parameters, same return types.
8. NEVER use markdown fences. Just raw code between markers.
9. Output the COMPLETE file, every line from first to last.";
