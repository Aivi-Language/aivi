import grammar from '../../tooling/packages/vscode-aivi/syntaxes/aivi.tmLanguage.json' with { type: 'json' }
import theme from './theme/aivi-dark-theme.json' with { type: 'json' }

// The site, documentation checker and highlighting tests share one registration.
export const markdownOptions = {
  languages: [{ ...grammar, aliases: ['aivi-fragment'] }],
  theme,
}
