type DomElement = Element;

declare namespace JSX {
	export type Element = DomElement;
	export interface IntrinsicElements {
		[name: string]: any;
	}
}