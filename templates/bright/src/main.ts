import './js/code-block.element.js';

import { initialiseNavHighlight } from './js/nav-highlight.js';

document.addEventListener('DOMContentLoaded', () => {
	let nav = document.querySelector('nav');
	if (nav != null) {
		initialiseNavHighlight(nav);
	}
});
