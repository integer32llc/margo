export type Tag = string;
export type Props = Record<string, unknown>;
export type Children = (Element | string)[];

function getTagNamespace(tag: Tag): string | undefined {
	switch (tag) {
		case 'svg':
		case 'path':
		case 'circle':
		case 'rect':
		case 'g':
			return 'http://www.w3.org/2000/svg';
		default:
			return undefined;
	}
}

export function h(
	tag: Tag,
	props: Props = {},
	...children: Children
): Element {
	// Create element
	let el: Element = document.createElement(tag);
	
	// If a namespace is given, or the tag belongs to a specific namespace,
	// we create the element in the given namespace.
	let ns = props.xmlns ?? getTagNamespace(tag);
	if (ns != undefined && typeof ns === 'string' && ns !== '') {
		el = document.createElementNS(ns, tag);
	}
	
	// Set element properties
	for (let [key, val] of Object.entries(props)) {
		if (key == 'className') {
			if (typeof val !== 'string') {
				throw new TypeError(`Class name must be a string, got ${typeof val}.`);
			}
			
			el.classList.add(...val.trim().split(' '));
			continue;
		}
		
		if (key.startsWith('on:')) {
			if (typeof val !== 'function') {
				throw new TypeError(`Event handler ${key} must be a function, got ${typeof val}.`);
			}
			
			let [_, event, ...modifiers] = key.split(':');
			el.addEventListener(event, val as any, {
				capture: modifiers.includes('capture') ? true : undefined,
				passive: modifiers.includes('passive') ? true : undefined,
				once: modifiers.includes('once') ? true : undefined,
			});
			
			continue;
		}
		
		if (typeof val !== 'string') {
			throw new TypeError(`Property ${key} must be a string, got ${typeof val}.`);
		}
		el.setAttribute(key, val);
	}
	
	// Append child elements into the parent
	children.forEach((child) => {
		el.append(child);
	});
	
	return el;
}
