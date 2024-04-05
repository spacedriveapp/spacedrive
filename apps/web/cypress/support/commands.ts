/// <reference types="cypress" />

import {
	libraryRegex,
	librarySettingsRegex,
	onboardingCreatingLibraryRegex,
	onboardingLibraryRegex,
	onboardingLocationRegex,
	onboardingPrivacyRegex,
	onboardingRegex
} from '../fixtures/routes';

declare global {
	namespace Cypress {
		interface Chainable {
			deleteLibrary(libraryName: string): Chainable<void>;
			fastOnboarding(libraryName: string): Chainable<void>;
			checkUrlIsLibrary(): Chainable<string>;
		}
	}
}

Cypress.Commands.add('checkUrlIsLibrary', () =>
	cy.url().should((url) => expect(libraryRegex.test(url)).to.be.true)
);

Cypress.Commands.add('fastOnboarding', (libraryName: string) => {
	// Initial alpha onboarding screen
	cy.visit('/');

	// Delete previous library if it exists
	cy.url()
		.should('match', new RegExp(`${libraryRegex.source}|${onboardingRegex.source}`))
		.then((url) => (onboardingRegex.test(url) ? url : cy.deleteLibrary(libraryName)));

	cy.get('a').contains('Continue').should('have.attr', 'href', '/onboarding/new-library').click();

	// Library name screen
	cy.url().should('match', onboardingLibraryRegex);
	cy.get('input[placeholder="e.g. \\"James\' Library\\""]').type(libraryName);
	cy.get('button').contains('New library').click();

	// Default locations screen
	cy.url().should('match', onboardingLocationRegex);
	cy.get('button').contains('Continue').click();

	// Privacy screen
	cy.url().should('match', onboardingPrivacyRegex);
	cy.get('label').contains('Share the bare minimum').click();
	cy.get('button[type="submit"]').contains('Continue').click();

	// Check redirect to create library screen
	cy.url().should('match', onboardingCreatingLibraryRegex);

	// Check redirect to Library
	cy.checkUrlIsLibrary();
});

Cypress.Commands.add('deleteLibrary', (libraryName: string) => {
	// Click on the library submenu
	cy.get('button[aria-haspopup="menu"]').contains(libraryName).click();
	cy.get('a').contains('Manage Library').click();

	// Check redirect to Library settings
	cy.url().should('match', librarySettingsRegex);

	// Check Library seetings screen title
	cy.get('h1').should('contain', 'Library Settings');

	// Check Library name is correct
	cy.get('label')
		.contains('Name')
		.parent()
		.find('input')
		.should((input) => {
			expect(input.val()).to.eq(libraryName);
		});

	// Delete Library
	cy.get('button').contains('Delete').click();

	// Check confirmation modal for deleting appears
	cy.get('body > div[role="dialog"]').as('deleteModal');

	// Check modal title
	cy.get('@deleteModal').find('h2').should('contain', 'Delete Library');

	cy.on('uncaught:exception', (err, runnable) => {
		// These errors are expected to occour right after the Library is deleted
		if (err.message.includes('Attempted to do library operation with no library set')) {
			return false;
		}
	});

	// Confirm delete
	cy.get('@deleteModal').find('button').contains('Delete').click();

	// After deleting a library check we are redirected back to onboarding);
	cy.url().should('match', onboardingRegex);
});

export {};
