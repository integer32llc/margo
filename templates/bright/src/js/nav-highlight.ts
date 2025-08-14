const ACTIVE_CLASS_NAME: string = 'nav-highlight';

interface NavLink {
	anchor: HTMLAnchorElement;
	target: Element;
}

/**
 * Enable navigation link highlighting for the given navigation element.
 *
 * Will find all `<a>` descendants of the given parent element and highlight
 * the anchor whose target element is currently the most prominent on screen.
 */
export function initialiseNavHighlight(navParent: Element): void {
	let links: NavLink[] = [];

	for (let anchor of Array.from(navParent.querySelectorAll('a'))) {
		// Parse the href and ditch any anchors that don't have a valid URL
		// or that don't refer to a local hash.
		let href = URL.parse(anchor.href);
		if (href == null || href.hash == '') {
			continue;
		}

		// The ID is simply the hash without the leading `#`.
		let id = href.hash.slice(1);
		let target = document.getElementById(id);
		if (target != undefined) {
			links.push({ anchor, target });
		}
	}
	
	// If there are no nav links to highlight, we don't need to run any more logic.
	if (links.length == 0) {
		return;
	}

	function updateHighlight() {
		// First we calculate the ratio each target element takes up on screen,
		// then we simply pick the element with the highest ratio.
		let ratios = links.map((link) => {
			let { top, bottom } = link.target.getBoundingClientRect();
			let ratio = (clamp(bottom) - clamp(top)) / window.innerHeight;
			return { anchor: link.anchor, ratio };
		});
		
		let activeAnchor: HTMLAnchorElement | null = null;
		if (ratios.length > 0) {
			activeAnchor = ratios.reduce((best, link) => {
				if (link.ratio > best.ratio) {
					return link;
				} else {
					return best;
				}
			}).anchor;
		}

		// Then all we need to do is update the nav anchors.
		for (let link of links) {
			if (link.anchor === activeAnchor) {
				link.anchor.classList.add(ACTIVE_CLASS_NAME);
			} else {
				link.anchor.classList.remove(ACTIVE_CLASS_NAME);
			}
		}
	}

	window.addEventListener('scroll', updateHighlight, {
		capture: true,
		passive: true,
	});
	
	updateHighlight();
}

/**
 * Clamp a number between 0 and the height of the window.
 */
function clamp(num: number): number {
	if (num < 0) {
		return 0;
	}
	if (num > window.innerHeight) {
		return window.innerHeight;
	}
	return num;
}
