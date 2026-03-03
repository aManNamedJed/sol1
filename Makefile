.PHONY: build pages clean

# Build the WebAssembly module
build:
	wasm-pack build --target web

# Prepare GitHub Pages deployment
pages: build
	@echo "Setting up GitHub Pages in docs/..."
	@mkdir -p docs
	@cp static/index.html docs/
	@cp pkg/sol1.js pkg/sol1_bg.wasm pkg/*.d.ts docs/ 2>/dev/null || true
	@touch docs/.nojekyll
	@sed -i.bak 's|from "../pkg/sol1.js"|from "./sol1.js"|g' docs/index.html && rm docs/index.html.bak
	@echo "✓ GitHub Pages ready in docs/"
	@echo "  Run: git add docs/ && git commit -m 'Update GitHub Pages' && git push"

# Clean build artifacts
clean:
	@rm -rf pkg/ target/ docs/
	@echo "✓ Cleaned build artifacts"
