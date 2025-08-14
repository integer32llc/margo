import { h } from './jsx.js';
import { createTimeline } from 'animejs';

class CodeBlockElement extends HTMLDivElement {
	private readonly _button = (
		<button
			on:click={this.copy.bind(this)}
			class="absolute top-0.5 right-0.5
				flex items-center gap-2 p-1.5 pl-2.5
				text-xs text-stone-400 rounded hover:bg-stone-100 hover:text-orange-500
				cursor-pointer duration-150 ease-in-out"
		>
			<span class="mb-0.5">Copy</span>
			<svg
				xmlns="http://www.w3.org/2000/svg"
				viewBox="0 0 24 24"
				fill="none"
				stroke="currentColor"
				stroke-width="2"
				stroke-linecap="round"
				stroke-linejoin="round"
				class="size-4"
			>
				<g data-name="copy">
					<rect width="14" height="14" x="8" y="8" rx="2" ry="2" />
					<path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2" />
				</g>

				<g data-name="success" style="opacity: 0; transform: rotate(180deg); transform-origin: 50% 50%;">
					<path d="M20 6 9 17l-5-5" />
				</g>
			</svg>
		</button>
	) as HTMLButtonElement;

	private _code: string = '';

	public constructor() {
		super();
	}

	public connectedCallback() {
		this._code = this.textContent ?? '';
		if (this._code == '') {
			return;
		}
		
		// Remove excess whitespace
		let whitespace = /^\n?([\t| ]+)/.exec(this._code);
		if (whitespace != null && whitespace.length > 1) {
			this._code = this._code.replaceAll(whitespace[1], '').trim();
		}
		
		this.textContent = this._code;

		this.style.position = 'relative';
		this.appendChild(this._button);
	}

	private async copy() {
		await navigator.clipboard.writeText(this._code);

		let label = this._button.querySelector('span')!;
		let copiedLabel = label.cloneNode() as HTMLSpanElement;
		copiedLabel.textContent = 'Copied';

		let labelRect = label.getBoundingClientRect();
		let buttonRect = this._button.getBoundingClientRect();

		console.log('label', labelRect, 'button', buttonRect);

		copiedLabel.style.position = 'absolute';
		copiedLabel.style.top = `${labelRect.top - buttonRect.top}px`;
		copiedLabel.style.right = `${buttonRect.right - labelRect.right}px`;
		label.insertAdjacentElement('afterend', copiedLabel);

		let icon = this._button.querySelector('svg')!;
		let copyIcon = icon.querySelector('g[data-name=copy]')!;
		let successIcon = icon.querySelector('g[data-name=success]')!;

		createTimeline()
			.set(this._button, { pointerEvents: 'none' })
			.set(icon, { rotate: 0 })
			.set(copyIcon, { opacity: 1 })
			.set(successIcon, { opacity: 0 })
			.set(label, { position: 'relative' })
			.set(copiedLabel, {
				opacity: 0,
				y: 20,
			})
			.call(() =>
				this._button.classList.add('!text-emerald-600', '!bg-white'),
			)
			.add(icon, {
				rotate: 180,
				duration: 150,
				ease: 'inOut(2)',
			})
			.add(
				copyIcon,
				{
					opacity: 0,
					duration: 120,
					ease: 'in(2)',
				},
				'<<',
			)
			.add(
				successIcon,
				{
					opacity: 1,
					delay: 30,
					duration: 120,
					ease: 'out(2)',
				},
				'<<',
			)
			.add(
				label,
				{
					y: -20,
					opacity: 0,
					duration: 120,
					ease: 'in(2)',
				},
				'<<',
			)
			.add(
				copiedLabel,
				{
					y: 0,
					opacity: 1,
					delay: 30,
					duration: 120,
					ease: 'out(2)',
				},
				'<<',
			)
			.add({ duration: 750 })
			.call(() =>
				this._button.classList.remove('!text-emerald-600', '!bg-white'),
			)
			.set(label, { y: 20 })
			.add(icon, {
				rotate: 360,
				duration: 150,
				ease: 'inOut(2)',
			})
			.add(
				successIcon,
				{
					opacity: 0,
					duration: 120,
					ease: 'in(2)',
				},
				'<<',
			)
			.add(
				copyIcon,
				{
					opacity: 1,
					delay: 30,
					duration: 120,
					ease: 'out(2)',
				},
				'<<',
			)
			.add(
				copiedLabel,
				{
					y: -20,
					opacity: 0,
					duration: 120,
					ease: 'in(2)',
				},
				'<<',
			)
			.add(
				label,
				{
					y: 0,
					opacity: 1,
					delay: 30,
					duration: 120,
					ease: 'out(2)',
				},
				'<<',
			)
			.call(() => copiedLabel.remove())
			.set(this._button, { pointerEvents: 'auto' })
			.play();
	}
}

window.customElements.define('code-block', CodeBlockElement, {
	extends: 'div',
});
