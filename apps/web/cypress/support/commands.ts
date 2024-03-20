/// <reference types="cypress" />

declare global {
	namespace Cypress {
		interface Chainable {
			deleteLibrary(libraryName: string): Chainable<void>;
			fastOnboarding(libraryName: string): Chainable<void>;
			checkUrlIsLibrary(): Chainable<string>;
		}
	}
}

const checkUrlIsLibrary = (url: string) =>
	/\/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\//.test(url);

Cypress.Commands.add('checkUrlIsLibrary', () =>
	cy.url().should((url) => expect(checkUrlIsLibrary(url)).to.be.true)
);

Cypress.Commands.add('fastOnboarding', (libraryName: string) => {
	// Initial alpha onboarding screen
	cy.visit('/');

	cy.url()
		.should(
			'match',
			/\/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\/|\/onboarding\/alpha$/
		)
		.then((url) => {
			if (url.endsWith('/onboarding/alpha')) return;

			cy.deleteLibrary(libraryName);

			return cy.visit('/');
		});

	cy.get('a').contains('Continue').should('have.attr', 'href', '/onboarding/new-library').click();

	// Library name screen
	cy.url().should('match', /\/onboarding\/new-library$/);
	cy.get('input[placeholder="e.g. \\"James\' Library\\""]').type(libraryName);
	cy.get('button').contains('New library').click();

	// Default locations screen
	cy.url().should('match', /\/onboarding\/locations$/);
	cy.get('button').contains('Continue').click();

	// Privacy screen
	cy.url().should('match', /\/onboarding\/privacy$/);
	cy.get('button[type="submit"]').contains('Continue').click();

	// Check redirect to Library
	cy.checkUrlIsLibrary();
});

Cypress.Commands.add('deleteLibrary', (libraryName: string) => {
	// Click on the library submenu
	cy.get('button[aria-haspopup="menu"]').contains(libraryName).click();
	cy.get('a').contains('Manage Library').click();

	// Check redirect to Library settings
	cy.url().should('match', /\/settings\/library\/general$/);

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

	// Confirm delete
	cy.get('@deleteModal').find('button').contains('Delete').click();
});

export {};
