# Discern : architecture

Éditeur SQL haute performance en Rust, UI GPUI.

## Principes

1. **Le cœur ne connaît pas l'UI.** Les dépendances vont toujours vers le bas.
2. **Pas de travail lourd sur le thread UI.** Lexing, parsing, requêtes et complétion tournent en arrière-plan sur des *snapshots* (`Buffer` est un rope, clone O(1)) et leurs résultats sont écartés si `revision` a changé.
3. **Tout est virtualisé.** L'éditeur ne peint que les lignes visibles ; la grille de résultats ne matérialise que les lignes visibles.
4. **Streaming.** Les résultats arrivent par `RowBatch` ; la première page s'affiche avant la fin de la requête.
5. **Chaque crate est testable seul**, sans fenêtre.

## Graphe de dépendances

```
discern-app (bin)
    └── discern-ui (GPUI)
            └── discern-core (workspace, documents, commandes)
                    ├── discern-text  (rope, edits, undo, sélection)
                    ├── discern-sql   (lexer, statements, dialectes)
                    └── discern-db    (traits Driver/Connection, types)
```

`discern-text`, `discern-sql` et `discern-db` sont indépendants entre eux.

## Responsabilités

| Crate | Rôle | Dépendances externes |
|---|---|---|
| `discern-text` | `Buffer` (ropey), `Edit`, `History`, `Selection`. Offsets en octets UTF-8. | ropey |
| `discern-sql` | `tokenize`, `split_statements`, `Dialect`. Fonctions pures sur `&str`. | thiserror |
| `discern-db` | `Driver`/`Connection`, `RowStream`, `Value`, `Column`. Aucun moteur concret. | async-trait, futures-core |
| `discern-core` | `Document` (buffer + tokens en cache), `Workspace` (onglets), `Command`. | crates internes |
| `discern-ui` | Vues GPUI : `editor`, `grid`, `sidebar`, `theme`. Aucune logique métier. | gpui |
| `discern-app` | Point d'entrée, câblage, config. | gpui_platform |

## Flux de données

```
Entrée utilisateur → Command → Workspace/Document (état) → notification → vue GPUI
                                   │
                                   └─ tâche async (snapshot + revision)
                                         ├─ re-lex / parse / complétion
                                         └─ Connection::execute → RowStream → grille
```

## Choix de performance

- Rope pour l'édition de gros scripts ; snapshots partageables entre threads.
- Lexer mono-passe, sans échec, travaillant sur les octets.
- Profil `dev` en `opt-level = 1` (dépendances en 3) : l'UI est inutilisable à 0.
- Profil `release` : LTO thin, `codegen-units = 1`.
- `unsafe_code = "forbid"` au niveau du workspace.
- `default-members` exclut le crate UI : `cargo test` reste rapide sans compiler GPUI.

## Ajouts prévus (par ordre)

1. `discern-sql` : lexing incrémental (fenêtre sale), parseur tolérant aux erreurs, complétion, formateur.
2. `discern-db` : drivers feature-gated (`postgres`, `mysql`, `sqlite`), pool, annulation, introspection du schéma (`Catalog`).
3. `discern-core` : exécuteur de requêtes (runtime async, annulation), historique de requêtes, persistance des sessions.
4. `discern-ui` : éditeur virtualisé avec cache de glyphes/lignes, grille de résultats, arbre de schéma, thèmes.
5. Benchmarks `criterion` (lexer, edits, grille) et CI (`fmt`, `clippy`, `test`).

## Commandes

```bash
cargo test                 # crates headless uniquement
cargo run -p discern-app   # lance l'application
```
