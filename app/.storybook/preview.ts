import type { Preview } from '@storybook/sveltekit'
import '../src/routes/layout.css'
import '../src/lib/styles/csfx-design-system.css'
import '../src/lib/styles/csfx-app.css'

const preview: Preview = {
  parameters: {
    controls: {
      matchers: {
       color: /(background|color)$/i,
       date: /Date$/i,
      },
    },
    a11y: {
      test: 'todo'
    }
  },
};

export default preview;