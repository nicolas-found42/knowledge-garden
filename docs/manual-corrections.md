# External manual corrections

Knowledge pages in `pages/` are ordinary Markdown. To correct a fact, change the displayed value on its generated row and keep the property name and following `Fact identity` line intact:

```markdown
- **duration:** 11 minutes
  - Fact identity: `fact-...`
```

Keep any generated qualifier suffix intact. Removing or changing it is preserved as uncertain content and does not create or replace manual authority. Backslash escapes emitted by the application for ASCII punctuation are decoded once when storing the corrected wording; literal backslashes use the generated doubled-backslash form. Dangling backslashes and escapes before non-punctuation fall outside this bounded grammar. This does not interpret arbitrary Markdown formatting. The application detects this supported form during automatic reconciliation or an index rescan. It records the value as an authoritative manual correction under that same fact identity. Later sources can change the fact's support, but cannot replace the owner's corrected value. Other facts on the page continue refreshing. The source's original quotation remains evidence of what the source said, separately from the owner's authority.

Owner prose and unknown YAML properties also survive regeneration. Prose that differs from the generated page is retained under Owner notes. This readable projection can change layout; it does not promise to infer arbitrary Markdown structure or meaning.

If a structural edit cannot be interpreted safely, the page shows an uncertain external-edit notice. The application retains the exact complete owner save, including whitespace and frontmatter, in a durable content-addressed Markdown file under `owner-edits/`. The page's `uninterpreted_owner_edits` YAML property lists paths relative to the collection root. These files are preservation material, separate from current facts and ordinary search indexes. Retain them along with the rest of the collection when backing it up or rebuilding indexes. Repeated rescans do not manufacture additional owner edits.

Changing a fact property label, deleting an identity, or editing generated evidence/relationships is outside the supported fact-correction grammar. Such changes are preserved with uncertainty rather than silently interpreted as corrections. Malformed frontmatter, page moves/renames, concurrent writers, and actual Obsidian interoperability have separate recovery and release validation requirements; the plain-file and same-directory atomic-save checks do not establish compatibility with every editor.
