const {test} = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const {JSDOM} = require('jsdom');
const React = require('react');
const {renderToStaticMarkup} = require('react-dom/server');
const axeSource = fs.readFileSync(require.resolve('axe-core/axe.min.js'), 'utf8');

// Server-rendered semantics only: this does not certify keyboard operation,
// contrast, screen readers, IME, loaded states or the native Electron shell.
test('Forge panel semantics pass automated WCAG checks on initial rendered states', async t => {
  const connect = async () => '';
  const noop = () => {};
  const cases = [
    ['GitPanel', 'git-panel', {connect, openPatch: async () => {}, onProposal: noop}],
    ['GroupsPanel', 'groups-panel', {connect, diff: async () => {}, revision: 0}],
    ['QualityPanel', 'quality-panel', {connect}],
    ['LanguagePanel', 'language-panel', {}],
    ['UpdatePanel', 'update-panel', {}],
    ['GraphView', 'graph-view', {
      graph: {nodes: ['src/a.ts', 'src/b.ts'], edges: [{from: 'src/a.ts', to: 'src/b.ts'}], unresolved: 0, truncated: false, elapsedMs: 1},
      selected: 'src/a.ts', onSelect: noop
    }]
  ];
  for (const [name, file, props] of cases) {
    await t.test(name, async () => {
      const Component = (await import('../packages/workbench/lib/browser/' + file + '.js'))[name];
      const api = new Proxy({}, {get() {
        return async () => { throw Error('Server rendering must not call backend APIs'); };
      }});
      const content = renderToStaticMarkup(React.createElement('main', null,
        React.createElement('h1', null, 'Forge Control'), React.createElement(Component, {...props, api})));
      const dom = new JSDOM('<!doctype html><html lang="es"><head><title>Forge accessibility fixture</title></head><body>' + content + '</body></html>', {runScripts: 'outside-only'});
      try {
        dom.window.eval(axeSource);
        const result = await dom.window.axe.run(dom.window.document, {
          runOnly: {type: 'tag', values: ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa']},
          // jsdom has no layout/paint implementation for contrast measurement.
          rules: {'color-contrast': {enabled: false}}
        });
        assert.equal(result.violations.length, 0, JSON.stringify(result.violations.map(v => ({
          id: v.id, impact: v.impact, nodes: v.nodes.map(n => n.html)
        }))));
      } finally {
        dom.window.close();
      }
    });
  }
});
