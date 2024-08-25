import { join } from 'path'
import colors from 'tailwindcss/colors'

const config = {
  // darkMode: 'class',
  content: [
    './src/lib/**/*.{html,svelte,ts}',
    './src/routes/**/*.{html,svelte,ts}',
    join(
      '../**/*.{html,svelte,js,ts}'
    )
  ],
  theme: {
    colors: {
      transparent: 'transparent',
      current: 'currentColor',
      white: colors.white,
      black: colors.black,
      pink: colors.pink,
      orange: colors.orange,
      amber: colors.amber,
      red: colors.red
    },
    extend: {}
  },
  plugins: [],
  safelist: []
}

export default config
